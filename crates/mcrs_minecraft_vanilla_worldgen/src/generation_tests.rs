use crate::generation_input::GenerationInput;
use crate::generation_stage::{CHUNK_TASK_POOL, GenerationStage, SchedulerConfig};
use bevy_tasks::{TaskPoolBuilder, block_on};
use mcrs_engine_link::{InlineEnd, Link, Placement, Stage as _};
use mcrs_minecraft_anvil::{ChunkStructures, SavedPiece};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
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
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

fn all_rows(y_sections: &[i32]) -> Range<i32> {
    let first = y_sections.first().copied().unwrap_or(0);
    let last = y_sections.last().copied().unwrap_or(first - 1);
    first..last + 1
}

fn generated_blocks(answer: GenerationAnswer) -> Option<BTreeMap<i32, ChunkBlocks>> {
    let stored = answer.stored?;
    let rows = stored.min_section_y..;
    Some(
        rows.zip(stored.sections)
            .map(|(y, section)| (y, section.blocks))
            .collect(),
    )
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Evict {
    Never,
    OnTicks,
    UnderARun,
}

#[derive(Clone, Copy, Debug)]
struct Drive {
    in_flight: usize,
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

#[expect(
    clippy::too_many_lines,
    reason = "stepping and read-back share one stage"
)]
fn run_parallel(dim: &Dimension, wanted: &[ColumnPos], drive: Drive) -> (Region, usize) {
    let y_sections = &dim.ctx.y_sections;
    CHUNK_TASK_POOL.get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().map_or(4, |p| p.get()))
            .build()
    });

    let mut end = InlineEnd::new(GenerationStage::new(
        SchedulerConfig {
            max_in_flight: drive.in_flight,
            max_dispatch_per_pass: drive.batch,
            ..Default::default()
        },
        all_rows(y_sections),
    ));
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
            end.receive(GenerationInput::Want(col));
        }

        for answer in end.step() {
            let column = answer.column;
            let Some(rows) = generated_blocks(answer) else {
                panic!("{drive:?} dropped {column:?} with no view sent");
            };
            generated.entry(column).or_default().extend(rows);
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
        let ended = end.wait();
        assert!(
            ended || !all_requested,
            "{drive:?} stopped with nothing on the pool before it generated the region"
        );
    }

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

fn run_writes(dim: &Dimension, wanted: &[ColumnPos]) -> (usize, usize) {
    let ctx = &dim.ctx;
    let mut filled: BTreeMap<ColumnPos, Arc<FilledSnapshot>> = BTreeMap::new();
    let fill = |col: ColumnPos| {
        Arc::new(
            fill_pooled(
                ctx,
                col,
                None,
                &mut AquiferCells::default(),
                &CancellationToken::new(),
            )
            .expect("the fill was not cancelled"),
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

const VILLAGE: Consumer = Consumer::Structure {
    id: "minecraft:village_plains",
    biome: "minecraft:plains",
};

const OUTPOST: Consumer = Consumer::Structure {
    id: "minecraft:pillager_outpost",
    biome: "minecraft:plains",
};

const DESERT_PYRAMID: Consumer = Consumer::Structure {
    id: "minecraft:desert_pyramid",
    biome: "minecraft:desert",
};

const BURIED_TREASURE: Consumer = Consumer::Structure {
    id: "minecraft:buried_treasure",
    biome: "minecraft:beach",
};

const FORTRESS: Consumer = Consumer::Structure {
    id: "minecraft:fortress",
    biome: "minecraft:nether_wastes",
};

const SHIPWRECK: Consumer = Consumer::Structure {
    id: "minecraft:shipwreck",
    biome: "minecraft:deep_frozen_ocean",
};

const OCEAN_RUIN: Consumer = Consumer::Structure {
    id: "minecraft:ocean_ruin_cold",
    biome: "minecraft:frozen_ocean",
};

const JUNGLE_TEMPLE: Consumer = Consumer::Structure {
    id: "minecraft:jungle_pyramid",
    biome: "minecraft:jungle",
};

const RUINED_PORTAL: Consumer = Consumer::Structure {
    id: "minecraft:ruined_portal",
    biome: "minecraft:plains",
};

const MONUMENT: Consumer = Consumer::Structure {
    id: "minecraft:monument",
    biome: "minecraft:deep_frozen_ocean",
};

const MINESHAFT: Consumer = Consumer::Structure {
    id: "minecraft:mineshaft",
    biome: "minecraft:plains",
};

const IGLOO: Consumer = Consumer::Structure {
    id: "minecraft:igloo",
    biome: "minecraft:snowy_plains",
};

const NETHER_FOSSIL: Consumer = Consumer::Structure {
    id: "minecraft:nether_fossil",
    biome: "minecraft:soul_sand_valley",
};

const STRONGHOLD: Consumer = Consumer::Structure {
    id: "minecraft:stronghold",
    biome: "minecraft:plains",
};

const SWAMP_HUT: Consumer = Consumer::Structure {
    id: "minecraft:swamp_hut",
    biome: "minecraft:swamp",
};

const END_CITY: Consumer = Consumer::Structure {
    id: "minecraft:end_city",
    biome: "minecraft:end_highlands",
};

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

fn want(link: &mut Link<GenerationStage>, col: ColumnPos) {
    link.send(GenerationInput::Want(col));
}

fn generated_rows(link: &mut Link<GenerationStage>) -> BTreeMap<i32, ChunkBlocks> {
    block_on(link.answered());
    let mut rows = BTreeMap::new();
    for answer in link.outputs() {
        let Some(blocks) = generated_blocks(answer) else {
            panic!("the stage dropped a column with no view sent");
        };
        rows.extend(blocks);
    }
    rows
}

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
    let rows = all_rows(&y_sections);
    let mut link = Link::new(
        GenerationStage::new(
            SchedulerConfig {
                max_in_flight: 64,
                max_dispatch_per_pass: 64,
                num_threads: 2,
            },
            rows.clone(),
        ),
        placement,
    );
    link.send(GenerationInput::Context(Box::new(ctx.clone())));

    want(&mut link, col);
    let generated = generated_rows(&mut link);
    assert_eq!(
        generated.keys().copied().collect::<Vec<i32>>(),
        rows.collect::<Vec<i32>>(),
        "{placement:?}: the ladder never generated the column"
    );

    let mut buffer = ColumnBlocks::new(&y_sections);
    let mut cells = AquiferCells::default();
    let alone = fill_column(
        &ctx,
        col,
        &mut buffer,
        None,
        &mut cells,
        &CancellationToken::new(),
    )
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
fn a_fill_on_a_busy_pool_takes_the_owed_columns_of_its_square() {
    use mcrs_minecraft_worldgen_generator::tests::{bare_fill_context, build_beta_router};

    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());
    let ctx = bare_fill_context(Arc::new(build_beta_router()));
    let square: Vec<ColumnPos> = (4..8)
        .flat_map(|x| (-8..-4).map(move |z| ColumnPos::new(x, z)))
        .collect();
    for (num_threads, filling) in [(1, 16), (64, 2)] {
        let mut end = InlineEnd::new(GenerationStage::new(
            SchedulerConfig {
                max_in_flight: 2,
                max_dispatch_per_pass: 2,
                num_threads,
            },
            all_rows(&ctx.y_sections),
        ));
        end.receive(GenerationInput::Context(Box::new(ctx.clone())));
        for &col in &square {
            end.receive(GenerationInput::Want(col));
        }
        end.receive(GenerationInput::Want(ColumnPos::new(8, -8)));
        assert_eq!(end.step().count(), 0, "no column is generated yet");

        let mut started: Vec<(i32, i32)> = end
            .stage()
            .in_flight
            .iter()
            .map(|stage| (stage.col.x, stage.col.z))
            .collect();
        started.sort_unstable();
        started.dedup();
        let mut expected: Vec<(i32, i32)> = square.iter().map(|col| (col.x, col.z)).collect();
        expected.sort_unstable();
        assert_eq!(started.len(), filling, "{num_threads} threads");
        assert!(
            started.iter().all(|col| expected.contains(col)),
            "{num_threads} threads: {started:?} are columns of the square"
        );
    }
}

#[test]
fn a_column_that_left_every_view_is_abandoned() {
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig::default(), 0..1),
        Placement::Inline,
    );
    let stale = ColumnPos::new(100, 100);
    want(&mut link, stale);
    link.send(GenerationInput::Views(vec![ChunkTrackingView {
        center: mcrs_voxel::SectionPos::new(0, 0, 0),
        distance: 2,
        vert_distance: 8,
        min_section_y: i32::MIN,
        max_section_y: i32::MAX,
    }]));
    block_on(link.answered());
    let abandoned: Vec<ColumnPos> = link
        .outputs()
        .filter(|answer| answer.stored.is_none())
        .map(|answer| answer.column)
        .collect();
    assert_eq!(abandoned, [stale], "a stale column is dropped");
}

#[test]
fn a_filled_column_lays_its_bedrock_floor() {
    use mcrs_minecraft_biome::source::BiomeSource;

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

    let y_sections = ctx.y_sections.clone();
    let mut column = ColumnBlocks::new(&y_sections);
    let mut cells = AquiferCells::default();
    let snapshot = fill_column(
        &ctx,
        col,
        &mut column,
        None,
        &mut cells,
        &CancellationToken::new(),
    )
    .expect("the fill was not cancelled");

    let bedrock = VoxelId::from(
        corpus()
            .default_state("minecraft:bedrock")
            .unwrap()
            .number(),
    );
    let (palette, _) = snapshot.sections[0]
        .as_ref()
        .expect("the bottom section carries blocks");
    let mut states = Vec::with_capacity(ColumnBlocks::SECTION_VOLUME);
    palette.0.for_each(|state| states.push(state));
    assert!(
        states[..256].iter().all(|state| *state == bedrock),
        "the bottom of the world is open: the material rules never ran over this dispatch"
    );
}

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
    let mut cells = AquiferCells::default();
    let snapshot = fill_column(
        &ctx,
        ColumnPos::new(3, -7),
        &mut column,
        None,
        &mut cells,
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

#[test]
fn a_generated_mineshaft_leaves_its_cave_spider_spawner_in_a_stored_column() {
    use mcrs_minecraft_block_entity::GeneratedBlockEntity;

    let dim = fill_context(MINESHAFT);
    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig::default(), all_rows(&dim.ctx.y_sections)),
        Placement::Inline,
    );
    link.send(GenerationInput::Context(Box::new(dim.ctx.clone())));
    let wanted: Vec<ColumnPos> = region_columns(1)
        .into_iter()
        .map(|col| ColumnPos::new(col.x + dim.centre.x, col.z + dim.centre.z))
        .collect();
    for col in &wanted {
        want(&mut link, *col);
    }
    block_on(link.answered());

    let stored: Vec<_> = link.outputs().filter_map(|answer| answer.stored).collect();
    assert_eq!(stored.len(), wanted.len(), "a wanted column was not stored");
    assert!(
        stored
            .iter()
            .any(|column| column.block_entities.iter().any(|entity| matches!(
                entity,
                GeneratedBlockEntity::MobSpawner { spawn_data: Some(data), .. }
                    if data.entity.get_string("id") == Some("minecraft:cave_spider")
            ))),
        "the mineshaft's spawner did not reach any stored column"
    );
}

fn generated_monument_column() -> (Dimension, GenerationAnswer) {
    let dim = fill_context(MONUMENT);
    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig::default(), all_rows(&dim.ctx.y_sections)),
        Placement::Inline,
    );
    link.send(GenerationInput::Context(Box::new(dim.ctx.clone())));
    want(&mut link, dim.centre);
    block_on(link.answered());
    let answer = link.outputs().next().expect("the column was answered");
    (dim, answer)
}

fn without_rooms(structures: &ChunkStructures) -> ChunkStructures {
    let mut cleared = structures.clone();
    for start in cleared.starts.values_mut() {
        for piece in &mut start.pieces {
            if let SavedPiece::OceanMonumentBuilding(tag) = piece {
                tag.rooms.clear();
                tag.children.clear();
            }
        }
    }
    cleared
}

#[test]
fn a_generated_monument_is_saved_and_its_rooms_are_rebuilt_from_the_seed_once() {
    let (dim, answer) = generated_monument_column();
    let column = answer.column;
    let structures = answer.stored.expect("the column was stored").structures;
    a_generated_monument_start_and_references_are_saved_with_the_column(&dim, column, &structures);
    a_saved_monument_without_its_rooms_gets_the_generated_graph_from_the_seed(
        &dim,
        column,
        &structures,
    );
    a_rebuilt_monument_marks_its_column_changed_once(&dim, column, &structures);
}

fn a_generated_monument_start_and_references_are_saved_with_the_column(
    dim: &Dimension,
    start_column: ColumnPos,
    structures: &ChunkStructures,
) {
    let id = ResourceLocationBuf::minecraft("monument").unwrap();
    assert_eq!(
        structures.starts.get(&id).map(|start| start.references),
        Some(0)
    );
    assert_eq!(
        structures.references.get(&id),
        Some(&BTreeSet::from([start_column]))
    );

    let neighbour = ColumnPos::new(start_column.x + 1, start_column.z);
    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());
    let mut link = Link::new(
        GenerationStage::new(SchedulerConfig::default(), all_rows(&dim.ctx.y_sections)),
        Placement::Inline,
    );
    link.send(GenerationInput::Context(Box::new(dim.ctx.clone())));
    want(&mut link, neighbour);
    block_on(link.answered());
    let touched = link
        .outputs()
        .find(|answer| answer.column == neighbour)
        .and_then(|answer| answer.stored)
        .expect("the neighbour was stored")
        .structures;
    assert!(touched.starts.is_empty());
    assert_eq!(
        touched.references.get(&id),
        Some(&BTreeSet::from([start_column]))
    );
}

fn a_saved_monument_without_its_rooms_gets_the_generated_graph_from_the_seed(
    dim: &Dimension,
    column: ColumnPos,
    generated: &ChunkStructures,
) {
    use crate::saved_monuments::rebuild_rooms;

    let monument = |structures: &ChunkStructures| {
        structures
            .starts
            .values()
            .flat_map(|start| start.pieces.iter())
            .find_map(|piece| match piece {
                SavedPiece::OceanMonumentBuilding(tag) => Some(tag.clone()),
                _ => None,
            })
    };
    let expected = monument(generated).expect("the start column holds the monument");
    assert!(!expected.rooms.is_empty() && !expected.children.is_empty());

    let mut saved = without_rooms(generated);
    let seed = dim.ctx.router.as_ref().unwrap().world_seed as i64;
    assert!(rebuild_rooms(&mut saved, column, seed));
    assert_eq!(monument(&saved), Some(expected));
    assert!(!rebuild_rooms(&mut saved, column, seed));
}

fn a_rebuilt_monument_marks_its_column_changed_once(
    dim: &Dimension,
    column: ColumnPos,
    generated: &ChunkStructures,
) {
    use crate::saved_monuments::rebuild_saved_monument_rooms;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_engine::stored_column::{
        ColumnRecord, ColumnStructures, PendingColumnFacts, SectionChanged, TakenFromStore,
    };
    use mcrs_minecraft_engine_world::world_seed::WorldSeed;
    use mcrs_voxel::SectionPos;

    let structures = without_rooms(generated);

    let mut world = World::new();
    world.insert_resource(WorldSeed(dim.ctx.router.as_ref().unwrap().world_seed as _));
    let owner = world.spawn_empty().id();
    let section = world
        .spawn((SectionPos::new(column.x, 0, column.z), TakenFromStore))
        .id();
    let mut pending = PendingColumnFacts::default();
    pending.0.insert(
        (owner, column),
        (ColumnStructures(structures), ColumnRecord::default()),
    );
    world.insert_resource(pending);

    world.run_system_once(rebuild_saved_monument_rooms).unwrap();
    assert!(world.entity(section).contains::<SectionChanged>());

    world.entity_mut(section).remove::<SectionChanged>();
    world.run_system_once(rebuild_saved_monument_rooms).unwrap();
    assert!(!world.entity(section).contains::<SectionChanged>());
}
