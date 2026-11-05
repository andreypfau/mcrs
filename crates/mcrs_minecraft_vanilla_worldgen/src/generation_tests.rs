use crate::generation_input::{GenerationInput, ViewUpdate, WantedColumn};
use crate::generation_output::{GenerationOutput, SectionContent};
use crate::generation_stage::{CHUNK_TASK_POOL, GenerationStage, SchedulerConfig};
use bevy_tasks::{TaskPoolBuilder, block_on};
use mcrs_engine_link::{InlineEnd, Link, Placement, Stage as _};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_worldgen_generator::ColumnBlocks;
use mcrs_minecraft_worldgen_generator::stages::{fill_pooled, run_region};
use mcrs_minecraft_worldgen_generator::staging::{
    FilledSnapshot, RegionSnapshots, Stage, region_column,
};
use mcrs_minecraft_worldgen_generator::task::CancellationToken;
use mcrs_minecraft_worldgen_generator::tests::corpus;
use mcrs_minecraft_worldgen_generator::tests::generate_region;
use mcrs_minecraft_worldgen_generator::tests::ladder::{
    Column, Consumer, Dimension, Region, decode, fill_context, region_columns,
};
use mcrs_minecraft_worldgen_generator::tests::surface::{
    fill_context as surface_fill_context, overworld_biome_registry, overworld_material_router,
};
use mcrs_voxel::ColumnPos;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The order a view asks for the columns in. Every one of them is unobservable
/// in the finished world, and each puts the stages of neighbouring columns into
/// a different interleaving.
#[derive(Clone, Copy, Debug)]
enum Order {
    Spiral,
    RowMajor,
    Shuffled(u64),
}

fn scramble(seed: u64, col: ColumnPos) -> u64 {
    let mut h = seed
        ^ (col.x as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (col.z as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^ (h >> 33)
}

fn ordered(cols: &[ColumnPos], order: Order) -> Vec<ColumnPos> {
    let mut cols = cols.to_vec();
    match order {
        Order::Spiral => cols.sort_by_key(|c| (c.x * c.x + c.z * c.z, c.z, c.x)),
        Order::RowMajor => cols.sort_by_key(|c| (c.z, c.x)),
        Order::Shuffled(seed) => cols.sort_by_key(|c| scramble(seed, *c)),
    }
    cols
}

/// When a drive forgets filled snapshots and the deltas beside them, so that
/// the columns climb the ladder a second time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Evict {
    Never,
    /// On fixed ticks, wherever a snapshot happens to be at `Filled`.
    OnTicks,
    /// Only where a neighbour has already run against the snapshot: the one
    /// state in which a delta can be lost between the eviction and the merge
    /// that owed it. Fired twice — once before any column of the region has
    /// merged, once after — because a column that has merged is a writer that
    /// has to run, and merge, a second time.
    UnderARun,
}

#[derive(Clone, Copy, Debug)]
struct Drive {
    /// Stages allowed on the pool at once: one, two, or what the machine has.
    in_flight: usize,
    /// Stages dispatched per step of the generation stage.
    batch: usize,
    order: Order,
    evict: Evict,
}

fn machine_parallelism() -> usize {
    SchedulerConfig::default().max_in_flight
}

fn default_batch() -> usize {
    SchedulerConfig::default().max_dispatch_per_pass
}

/// How many steps that find something to evict actually evict it. Bounded, or a
/// column evicted every step would never reach the top of its ladder — and counted
/// in steps that found something rather than in steps, because how far the ladder
/// has climbed by any given step is a question about the machine's load.
const EVICT_FIRINGS: usize = 6;

fn force_eviction(
    generation: &mut GenerationStage,
    wanted: &[ColumnPos],
    evict: Evict,
    after_a_merge: bool,
    rungs: usize,
) -> usize {
    let reach = 2 * rungs as i32;
    let halo: Vec<ColumnPos> = wanted
        .iter()
        .flat_map(|u| {
            (-reach..=reach).flat_map(move |dx| {
                (-reach..=reach).map(move |dz| ColumnPos::new(u.x + dx, u.z + dz))
            })
        })
        .collect();
    if after_a_merge
        && !wanted.iter().any(|col| {
            generation
                .store
                .stage(*col)
                .is_some_and(|at| at >= Stage::Merged(0))
        })
    {
        return 0;
    }
    let mut evicted = 0;
    for col in halo {
        if generation.is_in_flight(col) || generation.store.stage(col) != Some(Stage::Filled) {
            continue;
        }
        let under_a_run = (0..9).any(|slot| {
            generation
                .store
                .stage(region_column(col, slot))
                .is_some_and(|at| at >= Stage::Ran(0))
        });
        if evict == Evict::UnderARun && !under_a_run {
            continue;
        }
        generation.evict(col);
        evicted += 1;
    }
    evicted
}

/// Drive the real generation stage by hand, one step at a time, to the
/// generation of every wanted column, and read the blocks back off the
/// generated sections. Between its steps the stage waits for one ladder stage
/// on the pool to end, so the steps see the pool's own interleaving.
#[expect(
    clippy::too_many_lines,
    reason = "the helper drives the real stage to the top of every ladder and reads the blocks back, and the stepping and the read-back share one stage"
)]
fn run_parallel(dim: &Dimension, wanted: &[ColumnPos], drive: Drive) -> (Region, usize) {
    let y_sections = &dim.ctx.y_sections;
    // The pool is one per test binary, so its thread count is whichever test
    // reached it first; `in_flight` is what actually bounds the concurrency.
    CHUNK_TASK_POOL.get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().map_or(4, |p| p.get()))
            .build()
    });

    let mut end = InlineEnd::new(GenerationStage::new(SchedulerConfig {
        max_in_flight: drive.in_flight,
        max_dispatch_per_pass: drive.batch,
        ..Default::default()
    }));
    end.receive(GenerationInput::Context(Box::new(dim.ctx.clone())));

    let rungs = dim.ctx.rungs();
    let requests = ordered(wanted, drive.order);
    let mut generated: BTreeMap<ColumnPos, BTreeMap<i32, ChunkBlocks>> = BTreeMap::new();
    let mut next = 0usize;
    let mut evicted = 0usize;
    let mut firings = 0usize;

    loop {
        if let Some(&col) = requests.get(next) {
            next += 1;
            end.receive(GenerationInput::Want(WantedColumn {
                col,
                distance_sq: min_column_distance(&col, &[]),
                rows: y_sections.to_vec(),
            }));
        }

        for output in end.step() {
            let GenerationOutput::Generated(column) = output else {
                panic!("{drive:?} abandoned a column with no view sent");
            };
            let rows = generated.entry(column.col).or_default();
            for section in column.sections {
                let SectionContent::Blocks(content) = section.content else {
                    panic!("{drive:?} generated {:?} without its blocks", column.col);
                };
                rows.insert(section.y, content.blocks);
            }
        }

        let generation = end.stage_mut();
        match drive.evict {
            Evict::OnTicks if firings < EVICT_FIRINGS => {
                let n = force_eviction(generation, wanted, drive.evict, false, rungs);
                if n > 0 {
                    firings += 1;
                    evicted += n;
                }
            }
            Evict::UnderARun if firings < 2 => {
                let n = force_eviction(generation, wanted, drive.evict, firings == 1, rungs);
                if n > 0 {
                    firings += 1;
                    evicted += n;
                }
            }
            _ => {}
        }

        let all_requested = next == requests.len();
        if all_requested
            && requests.iter().all(|col| {
                generated
                    .get(col)
                    .is_some_and(|rows| y_sections.iter().all(|y| rows.contains_key(y)))
            })
        {
            break;
        }
        if end.stage().has_work() {
            continue;
        }
        let ended = block_on(end.wait());
        assert!(
            ended.is_some() || !all_requested,
            "{drive:?} stopped with nothing on the pool before it generated the region"
        );
    }

    // What a column was generated with is what it stays. The merged blocks
    // outlive the emission — a section ticketed later is generated from them —
    // so a re-merge after an eviction has to land on the same column.
    let standing: Vec<(ColumnPos, Column)> = wanted
        .iter()
        .filter_map(|col| {
            let merged = end.stage().store.merged(*col)?;
            Some((*col, decode(&merged.sections)))
        })
        .collect();

    let region: Region = requests
        .iter()
        .map(|&col| {
            let column = y_sections
                .iter()
                .map(|y| {
                    let blocks = generated
                        .get(&col)
                        .and_then(|rows| rows.get(y))
                        .unwrap_or_else(|| panic!("{col:?} was generated without its blocks"));
                    (0..ColumnBlocks::SECTION_VOLUME)
                        .map(|index| {
                            blocks
                                .get_cell(index & 15, index >> 8, (index >> 4) & 15)
                                .unwrap()
                                .into()
                        })
                        .collect()
                })
                .collect();
            (col, column)
        })
        .collect();

    for (col, merged) in standing {
        let landed = region.get(&col).expect("every wanted column was generated");
        assert_eq!(
            merged.len(),
            landed.len(),
            "{drive:?}: the merged {col:?} carries a different number of sections"
        );
        for (slot, (merged, landed)) in merged.iter().zip(landed.iter()).enumerate() {
            assert!(
                merged == landed,
                "{drive:?}: the merged {col:?} left section slot {slot} \
                 unlike the one it generated"
            );
        }
    }
    (region, evicted)
}

fn run_oracle(dim: &Dimension, wanted: &[ColumnPos]) -> Region {
    let min = ColumnPos::new(
        wanted.iter().map(|c| c.x).min().unwrap(),
        wanted.iter().map(|c| c.z).min().unwrap(),
    );
    let max = ColumnPos::new(
        wanted.iter().map(|c| c.x).max().unwrap(),
        wanted.iter().map(|c| c.z).max().unwrap(),
    );
    generate_region(&dim.ctx, min, max)
        .into_iter()
        .map(|(col, merged)| (col, decode(&merged.sections)))
        .collect()
}

#[derive(Default)]
struct Census {
    compared: usize,
    differ: BTreeMap<(u16, u16), usize>,
    first: Option<String>,
}

impl Census {
    fn total(&self) -> usize {
        self.differ.values().sum()
    }

    fn take(&mut self, want: &Region, got: &Region, label: &str) {
        for (col, want_column) in want {
            let got_column = got
                .get(col)
                .unwrap_or_else(|| panic!("{label}: {col:?} never arrived"));
            assert_eq!(
                want_column.len(),
                got_column.len(),
                "{label}: {col:?} carries a different number of sections"
            );
            for (slot, (want_cells, got_cells)) in
                want_column.iter().zip(got_column.iter()).enumerate()
            {
                for index in 0..want_cells.len() {
                    let (x, y, z) = (index & 15, index >> 8, (index >> 4) & 15);
                    self.compared += 1;
                    let (want, got) = (want_cells[index], got_cells[index]);
                    if want == got {
                        continue;
                    }
                    *self.differ.entry((want.0, got.0)).or_default() += 1;
                    self.first.get_or_insert_with(|| {
                        let (cx, cz) = (col.x, col.z);
                        format!(
                            "{label}: column ({cx}, {cz}) section slot {slot} at \
                             ({x}, {y}, {z}): want {want:?}, got {got:?}"
                        )
                    });
                }
            }
        }
    }
}

fn small_drives() -> Vec<Drive> {
    vec![
        Drive {
            in_flight: 1,
            batch: 1,
            order: Order::Spiral,
            evict: Evict::Never,
        },
        Drive {
            in_flight: 2,
            batch: 1,
            order: Order::RowMajor,
            evict: Evict::Never,
        },
        Drive {
            in_flight: machine_parallelism(),
            batch: default_batch(),
            order: Order::Spiral,
            evict: Evict::Never,
        },
        Drive {
            in_flight: machine_parallelism(),
            batch: default_batch(),
            order: Order::RowMajor,
            evict: Evict::OnTicks,
        },
        Drive {
            in_flight: 2,
            batch: default_batch(),
            order: Order::Shuffled(0x5EED),
            evict: Evict::OnTicks,
        },
        Drive {
            in_flight: machine_parallelism(),
            batch: 1,
            order: Order::Shuffled(7),
            evict: Evict::OnTicks,
        },
        // The lost delta: a column dropped after a neighbour ran against it
        // must come back with that neighbour's writes, and one stage on the
        // pool at a time is what lets the eviction find a snapshot at `Filled`
        // beside a column that has run.
        Drive {
            in_flight: 1,
            batch: 1,
            order: Order::RowMajor,
            evict: Evict::UnderARun,
        },
        Drive {
            in_flight: 2,
            batch: default_batch(),
            order: Order::Shuffled(0xD317A),
            evict: Evict::UnderARun,
        },
    ]
}

/// What `Run` actually produced over a region: how many of the deltas the
/// units emitted carried a write, and how many of those landed in a column
/// other than the one that ran.
///
/// The oracle-against-parallel comparison is only worth anything while both
/// sides move blocks. `Run` was a no-op when this file was written, every delta
/// was empty, and the comparison was two empty pipelines against each other; it
/// must never be able to fall back to that unnoticed.
fn run_writes(dim: &Dimension, wanted: &[ColumnPos]) -> (usize, usize) {
    let ctx = &dim.ctx;
    let mut filled: BTreeMap<ColumnPos, Arc<FilledSnapshot>> = BTreeMap::new();
    let fill = |col: ColumnPos| {
        Arc::new(
            fill_pooled(ctx, col, &CancellationToken::new()).expect("the fill was not cancelled"),
        )
    };
    let (mut carried, mut crossed) = (0usize, 0usize);
    for &col in wanted {
        let snapshots: RegionSnapshots = std::array::from_fn(|slot| {
            let at = region_column(col, slot);
            filled.entry(at).or_insert_with(|| fill(at)).clone()
        });
        for rung in 0..ctx.rungs() {
            for (target, delta) in run_region(ctx, &snapshots, rung) {
                if delta.writes.is_empty() {
                    continue;
                }
                carried += 1;
                crossed += usize::from(target != col);
            }
        }
    }
    (carried, crossed)
}

/// A plains village: jigsaw pieces that straddle columns.
const VILLAGE: Consumer = Consumer::Structure {
    id: "minecraft:village_plains",
    biome: "minecraft:plains",
};

/// A pillager outpost, the other jigsaw structure a plains column starts; its
/// pieces stay inside the clip of the column that runs.
const OUTPOST: Consumer = Consumer::Structure {
    id: "minecraft:pillager_outpost",
    biome: "minecraft:plains",
};

/// A desert pyramid: one grid piece over four columns, dug fourteen blocks
/// below its floor, with chests and the after-place sand, every write clipped
/// to the column that runs.
const DESERT_PYRAMID: Consumer = Consumer::Structure {
    id: "minecraft:desert_pyramid",
    biome: "minecraft:desert",
};

/// A buried treasure: one block of one column, dug down from the ocean floor
/// to the chest's resting block, with the fill around it never leaving the
/// column that runs.
const BURIED_TREASURE: Consumer = Consumer::Structure {
    id: "minecraft:buried_treasure",
    biome: "minecraft:beach",
};

/// A nether fortress: a tree of grid pieces with bridges reaching over many
/// columns, blaze spawners and chests, every write clipped to the column that
/// runs.
const FORTRESS: Consumer = Consumer::Structure {
    id: "minecraft:fortress",
    biome: "minecraft:nether_wastes",
};

/// A shipwreck: one template piece lowered to the ocean floor at layout, its
/// chests seeded through data markers, every write clipped to the column that
/// runs.
const SHIPWRECK: Consumer = Consumer::Structure {
    id: "minecraft:shipwreck",
    biome: "minecraft:deep_frozen_ocean",
};

/// A cold ocean ruin: three rotted templates over one another on the ocean
/// floor, a cluster of smaller ones around them when the start is large, with
/// chests and drowned at their markers, every write clipped to the column
/// that runs.
const OCEAN_RUIN: Consumer = Consumer::Structure {
    id: "minecraft:ocean_ruin_cold",
    biome: "minecraft:frozen_ocean",
};

/// A jungle temple: one grid piece over four columns, its cellar dug four
/// blocks below its floor, with dispensers, chests and a tripwire, every write
/// clipped to the column that runs.
const JUNGLE_TEMPLE: Consumer = Consumer::Structure {
    id: "minecraft:jungle_pyramid",
    biome: "minecraft:jungle",
};

/// A ruined portal: one template written whole from the column holding its
/// centre, then netherrack spread fourteen blocks around it, so the writes
/// reach the ring from a single column.
const RUINED_PORTAL: Consumer = Consumer::Structure {
    id: "minecraft:ruined_portal",
    biome: "minecraft:plains",
};

/// An ocean monument: one building piece over sixteen columns with its rooms
/// in memory, water boxes read before they are written, pillars filled down
/// to the floor and elder guardians spawned, every write clipped to the
/// column that runs.
const MONUMENT: Consumer = Consumer::Structure {
    id: "minecraft:monument",
    biome: "minecraft:deep_frozen_ocean",
};

/// A mineshaft: corridors, crossings and stairs branching from a room below
/// sea level, with supports read off the column's own blocks, chest minecarts
/// and cave spider spawners, every write clipped to the column that runs.
const MINESHAFT: Consumer = Consumer::Structure {
    id: "minecraft:mineshaft",
    biome: "minecraft:plains",
};

/// An igloo: three templates stacked down a shaft, lowered to the terrain under
/// the entrance at layout, with the laboratory's chest and its villagers. Its
/// only write past the clip is the snow over the trapdoor from a neighbour the
/// top straddles, which the start this seed finds does not.
const IGLOO: Consumer = Consumer::Structure {
    id: "minecraft:igloo",
    biome: "minecraft:snowy_plains",
};

/// A nether fossil: one template piece written whole by every column it
/// touches, so the columns around it carry its blocks into the ring.
const NETHER_FOSSIL: Consumer = Consumer::Structure {
    id: "minecraft:nether_fossil",
    biome: "minecraft:soul_sand_valley",
};

/// A stronghold: a tree of grid pieces buried below sea level with its
/// portal room, chests and silverfish spawner, every write clipped to the
/// column that runs.
const STRONGHOLD: Consumer = Consumer::Structure {
    id: "minecraft:stronghold",
    biome: "minecraft:plains",
};

/// A swamp hut: one grid piece inside its own column, raised to the ground,
/// with the witch and the cat it spawns.
const SWAMP_HUT: Consumer = Consumer::Structure {
    id: "minecraft:swamp_hut",
    biome: "minecraft:swamp",
};

/// An end city: dozens of template pieces over many columns, each write and
/// each marker's chest seed, shulker or elytra frame clipped to the column
/// that runs.
const END_CITY: Consumer = Consumer::Structure {
    id: "minecraft:end_city",
    biome: "minecraft:end_highlands",
};

/// A woodland mansion: hundreds of template pieces over some fifty columns,
/// their chests and the mobs its markers spawn, the cobblestone footing under
/// each column, every write clipped to the column that runs.
const MANSION: Consumer = Consumer::Structure {
    id: "minecraft:mansion",
    biome: "minecraft:dark_forest",
};

fn assert_region_agrees(consumer: Consumer, radius: i32, drives: &[Drive]) {
    let dim = fill_context(consumer);
    let y_sections = &dim.ctx.y_sections;
    let wanted: Vec<ColumnPos> = region_columns(radius)
        .into_iter()
        .map(|col| ColumnPos::new(col.x + dim.centre.x, col.z + dim.centre.z))
        .collect();
    let oracle = run_oracle(&dim, &wanted);
    assert_eq!(
        oracle.len(),
        wanted.len(),
        "the oracle skipped a column of the region"
    );

    let (carried, crossed) = run_writes(&dim, &wanted);
    assert!(
        carried > 0,
        "{consumer:?} wrote nothing anywhere: the comparison below would hold \
         between two pipelines that both do nothing"
    );
    if consumer != OUTPOST
        && consumer != DESERT_PYRAMID
        && consumer != BURIED_TREASURE
        && consumer != FORTRESS
        && consumer != SHIPWRECK
        && consumer != OCEAN_RUIN
        && consumer != JUNGLE_TEMPLE
        && consumer != MONUMENT
        && consumer != MINESHAFT
        && consumer != IGLOO
        && consumer != STRONGHOLD
        && consumer != SWAMP_HUT
        && consumer != END_CITY
        && consumer != MANSION
    {
        assert!(
            crossed > 0,
            "{consumer:?} wrote only into the columns that ran: nothing reaches the \
             ring, so no ordering the drives vary is observable"
        );
    } else {
        assert_eq!(
            crossed, 0,
            "{consumer:?} wrote past the clip of the column that ran"
        );
    }

    let mut census = Census::default();
    for drive in drives {
        let (parallel, evicted) = run_parallel(&dim, &wanted, *drive);
        assert!(
            drive.evict == Evict::Never || evicted > 0,
            "{drive:?} asked for an eviction and never found a snapshot to evict"
        );
        census.take(&oracle, &parallel, &format!("{consumer:?} {drive:?}"));
    }

    let per_column = y_sections.len() * ColumnBlocks::SECTION_VOLUME;
    assert_eq!(
        census.compared,
        wanted.len() * per_column * drives.len(),
        "a run went uncompared"
    );
    assert!(
        census.differ.is_empty(),
        "{} of {} blocks differ: {:?}\nfirst {:?}",
        census.total(),
        census.compared,
        census.differ,
        census.first
    );
}

fn want_rows(link: &mut Link<GenerationStage>, col: ColumnPos, rows: &[i32]) {
    link.send(GenerationInput::Want(WantedColumn {
        col,
        distance_sq: 0,
        rows: rows.to_vec(),
    }));
}

/// Waits for the stage to answer, and returns the blocks of each section it generated by row.
fn generated_rows(link: &mut Link<GenerationStage>) -> BTreeMap<i32, ChunkBlocks> {
    block_on(link.answered());
    let mut rows = BTreeMap::new();
    for output in link.outputs() {
        let GenerationOutput::Generated(column) = output else {
            panic!("the stage abandoned a column with no view sent");
        };
        for section in column.sections {
            let SectionContent::Blocks(content) = section.content else {
                panic!("row {} was generated without its blocks", section.y);
            };
            rows.insert(section.y, content.blocks);
        }
    }
    rows
}

/// The ladder must produce the column the fill alone produces: with no features to run, every
/// delta is empty and the merge is the identity. This drives the whole of it — the derived 5x5
/// of fills, the 3x3 of runs, the merge and the emission — through each placement of the stage,
/// so a readiness or budget bug shows up as a column that never arrives.
#[test]
fn the_ladder_generates_the_column_the_fill_produced() {
    for placement in [Placement::Inline, Placement::Thread] {
        the_ladder_generates_the_filled_column(placement);
    }
}

fn the_ladder_generates_the_filled_column(placement: Placement) {
    use mcrs_minecraft_worldgen_generator::stages::fill_column;
    use mcrs_minecraft_worldgen_generator::tests::{bare_fill_context, build_beta_router};

    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());

    let col = ColumnPos::new(1, -2);
    let ctx = bare_fill_context(Arc::new(build_beta_router()));
    let y_sections = ctx.y_sections.clone();
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig {
            max_in_flight: 64,
            max_dispatch_per_pass: 64,
            num_threads: 2,
        }),
        placement,
    );
    link.send(GenerationInput::Context(Box::new(ctx.clone())));

    let carried = [-1, 0, 4];
    want_rows(&mut link, col, &carried);
    let mut generated = generated_rows(&mut link);
    assert_eq!(
        generated.keys().copied().collect::<Vec<i32>>(),
        carried,
        "{placement:?}: the ladder never generated the column"
    );

    // A row asked for after the column was generated must come from the same merged blocks,
    // not from the snapshot the neighbours read.
    want_rows(&mut link, col, &[5]);
    let late = generated_rows(&mut link);
    assert!(
        late.contains_key(&5),
        "{placement:?}: a row asked for after the column was generated never landed"
    );
    generated.extend(late);

    let mut buffer = ColumnBlocks::new(&y_sections);
    let alone = fill_column(&ctx, col, &mut buffer, &CancellationToken::new())
        .expect("the fill was not cancelled");
    let bottom = y_sections[0];
    for (y, got) in &generated {
        let want = alone.sections[(y - bottom) as usize]
            .as_ref()
            .expect("the fill produced this section");
        for cell in 0..ColumnBlocks::SECTION_VOLUME {
            let local = mcrs_voxel::LocalPos::from_index(cell);
            let (x, ly, z) = (local.x() as usize, local.y() as usize, local.z() as usize);
            assert_eq!(
                got.get_cell(x, ly, z).map(VoxelId::from),
                want.0.get_cell(x, ly, z),
                "{placement:?}: section {y} diverged at ({x}, {ly}, {z})"
            );
        }
    }
}

#[test]
fn a_column_that_left_every_view_is_abandoned() {
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig::default()),
        Placement::Inline,
    );
    let stale = ColumnPos::new(100, 100);
    want_rows(&mut link, stale, &[0]);
    link.send(GenerationInput::Views(ViewUpdate {
        views: vec![ChunkTrackingView {
            center: mcrs_voxel::SectionPos::new(0, 0, 0),
            distance: 2,
            vert_distance: 8,
            min_section_y: i32::MIN,
            max_section_y: i32::MAX,
        }],
        players: Vec::new(),
    }));
    block_on(link.answered());
    let abandoned: Vec<ColumnPos> = link
        .outputs()
        .filter_map(|output| match output {
            GenerationOutput::Abandoned(column) => Some(column),
            GenerationOutput::Generated(_) => None,
        })
        .collect();
    assert_eq!(abandoned, [stale], "a stale column is dropped");
}

/// A generated column owes only the sections it was asked for — the ticket layer caps
/// how many it spawns a tick and cuts a column's sections across two of them —
/// while the fill covers the whole dimension, because the bedrock floor is a
/// material rule and a column surfaced over a slice would be left open at the
/// bottom.
#[test]
fn a_column_generated_for_part_of_its_sections_still_lays_its_bedrock_floor() {
    use mcrs_minecraft_biome::source::BiomeSource;

    use crate::generation_stage::carried_sections;
    use mcrs_minecraft_worldgen_generator::stages::fill_column;
    use mcrs_minecraft_worldgen_generator::task::CancellationToken;

    let (registry, ids) = overworld_biome_registry();
    let (router, material) = overworld_material_router(2, &ids);
    let ctx = surface_fill_context(
        router,
        material,
        registry,
        BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(
            mcrs_minecraft_worldgen_generator::tests::parameter_list_id("minecraft:overworld"),
        )),
    );

    let col = ColumnPos::new(3, -7);
    let carried = [-4, 3, 4];

    let y_sections = ctx.y_sections.clone();
    let mut column = ColumnBlocks::new(&y_sections);
    let snapshot = fill_column(&ctx, col, &mut column, &CancellationToken::new())
        .expect("the fill was not cancelled");

    let bottom = y_sections[0];
    let landed: Vec<_> = carried_sections(&carried, &snapshot.sections, bottom).collect();
    assert_eq!(
        landed.len(),
        carried.len(),
        "the generated column returned sections it was not asked for"
    );
    let bedrock = VoxelId::from(
        corpus()
            .default_state("minecraft:bedrock")
            .unwrap()
            .number(),
    );
    let floor = landed
        .iter()
        .find(|(y, _)| *y == carried[0])
        .expect("the bottom section came back");
    let (palette, _) = floor.1.expect("the bottom section carries blocks");
    let mut states = Vec::with_capacity(ColumnBlocks::SECTION_VOLUME);
    palette.0.for_each(|state| states.push(state));
    // The palette runs y, then z, then x, so the first layer is the floor of
    // the dimension, which `bedrock_floor` covers whole.
    assert!(
        states[..256].iter().all(|state| *state == bedrock),
        "the bottom of the world is open: the material rules never ran over this dispatch"
    );
}

/// The biome bytes a column stores are the ids the loader numbered its biomes
/// with, the numbering the registry packet is projected from, so a palette a
/// client receives names the biome the source chose.
#[test]
fn a_fixed_source_stores_the_ids_the_loader_numbered_its_biomes_with() {
    use mcrs_minecraft_biome::source::BiomeSource;
    use mcrs_minecraft_registries::{Named, Resolve, Resolver};

    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_worldgen_generator::stages::fill_column;

    let registries = test_registries().clone();
    let biomes = registries
        .entries::<mcrs_minecraft_biome::Biome>()
        .expect("the loader parses the biomes");
    let ids: std::collections::HashMap<String, u16> = biomes
        .names()
        .iter()
        .zip(0..)
        .map(|(name, id)| (name.to_string(), id))
        .collect();
    let (router, material) = overworld_material_router(2, &ids);
    let source = BiomeSource::resolve(
        serde_json::from_str::<BiomeSource<Named>>(
            r#"{"type":"minecraft:fixed","biome":"minecraft:desert"}"#,
        )
        .expect("a fixed biome source parses"),
        &registries,
    )
    .expect("the loader holds desert");
    let desert = Resolver::id::<Biome>(
        &registries,
        &ResourceKeyBuf::from_location(ResourceLocationBuf::read("minecraft:desert").unwrap()),
    )
    .expect("the loader holds desert")
    .narrow::<u8>()
    .expect("a biome id the palette can store");
    let ctx = surface_fill_context(router, material, registries.clone(), source);

    let mut column = ColumnBlocks::new(&ctx.y_sections);
    let snapshot = fill_column(
        &ctx,
        ColumnPos::new(3, -7),
        &mut column,
        &CancellationToken::new(),
    )
    .expect("the fill was not cancelled");

    let mut cells = 0;
    for (_, biomes) in snapshot.sections.iter().flatten() {
        for (x, y, z) in
            (0..16).flat_map(|x| (0..16).flat_map(move |y| (0..16).map(move |z| (x, y, z))))
        {
            assert_eq!(biomes.get_cell(x, y, z), Some(desert));
            cells += 1;
        }
    }
    assert!(cells > 0, "the column stored no biome cell");
}

#[test]
fn the_parallel_ladder_generates_ores_that_write_across_columns() {
    let drive = Drive {
        in_flight: 2,
        batch: default_batch(),
        order: Order::Shuffled(0xD317A),
        evict: Evict::UnderARun,
    };
    assert_region_agrees(Consumer::ModernOre, 1, &[drive]);
}

mod exhaustive {
    use super::*;

    /// The nine columns around the origin, through every worker count, request
    /// order, batch size and eviction the definition leaves free, for each program
    /// `Run` has.
    #[test]
    fn the_parallel_ladder_generates_the_oracle_region() {
        for consumer in [
            Consumer::BetaOre,
            Consumer::ModernOre,
            Consumer::Tree,
            Consumer::Corpus,
            VILLAGE,
            OUTPOST,
            DESERT_PYRAMID,
            BURIED_TREASURE,
            FORTRESS,
            SHIPWRECK,
            OCEAN_RUIN,
            JUNGLE_TEMPLE,
            RUINED_PORTAL,
            MONUMENT,
            MINESHAFT,
            IGLOO,
            NETHER_FOSSIL,
            STRONGHOLD,
            SWAMP_HUT,
            END_CITY,
            MANSION,
        ] {
            assert_region_agrees(consumer, 1, &small_drives());
        }
    }

    /// The same comparison over forty-nine wanted columns — a hundred and
    /// twenty-one fills per run, so the interior columns are further from the edge
    /// of the region than any dependency reaches.
    #[test]
    fn the_parallel_ladder_generates_the_large_oracle_region() {
        for consumer in [
            Consumer::BetaOre,
            Consumer::ModernOre,
            Consumer::Tree,
            Consumer::Corpus,
            VILLAGE,
        ] {
            assert_region_agrees(consumer, 3, &small_drives());
        }
    }
}
