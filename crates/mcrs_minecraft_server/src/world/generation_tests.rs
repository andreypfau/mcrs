use crate::world::chunk::{
    CHUNK_TASK_POOL, ColumnScheduler, SchedulerConfig, deliver_merged_columns,
    dispatch_column_generation, enqueue_pending_columns, process_completed_columns,
    request_section,
};
use crate::world::heightmap::PendingColumnHeightmaps;
use bevy_app::{App, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_tasks::TaskPoolBuilder;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::lifecycle::stage::{SectionStage, SectionStageChanged};
use mcrs_minecraft_protocol::ColumnPos;
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
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Stages dispatched per tick.
    batch: usize,
    order: Order,
    evict: Evict,
}

fn machine_parallelism() -> usize {
    SchedulerConfig::default().max_in_flight
}

fn default_batch() -> usize {
    SchedulerConfig::default().max_dispatch_per_tick
}

/// How many ticks that find something to evict actually evict it. Bounded, or a
/// column evicted every tick would never reach the delivery — and counted in
/// ticks that found something rather than in ticks, because how far the ladder
/// has climbed by any given tick is a question about the machine's load.
const EVICT_FIRINGS: usize = 6;

fn force_eviction(
    app: &mut App,
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
    let mut scheduler = app.world_mut().resource_mut::<ColumnScheduler>();
    if after_a_merge
        && !wanted.iter().any(|col| {
            scheduler
                .store
                .stage(*col)
                .is_some_and(|at| at >= Stage::Merged(0))
        })
    {
        return 0;
    }
    let mut evicted = 0;
    for col in halo {
        if scheduler.is_in_flight(col) || scheduler.store.stage(col) != Some(Stage::Filled) {
            continue;
        }
        let under_a_run = (0..9).any(|slot| {
            scheduler
                .store
                .stage(region_column(col, slot))
                .is_some_and(|at| at >= Stage::Ran(0))
        });
        if evict == Evict::UnderARun && !under_a_run {
            continue;
        }
        scheduler.store.forget(col);
        evicted += 1;
    }
    evicted
}

/// Drive the real scheduler — the same four systems the dimension runs — to the
/// delivery of every wanted column, and read the blocks back off the delivered
/// sections.
fn run_parallel(dim: &Dimension, wanted: &[ColumnPos], drive: Drive) -> (Region, usize) {
    let y_sections = &dim.ctx.y_sections;
    // The pool is one per test binary, so its thread count is whichever test
    // reached it first; `in_flight` is what actually bounds the concurrency.
    CHUNK_TASK_POOL.get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().map_or(4, |p| p.get()))
            .build()
    });

    let mut app = App::new();
    dim.install(&mut app);
    app.add_message::<SectionStageChanged>();
    let dimension_entity = app.world_mut().spawn_empty().id();
    app.init_resource::<PendingColumnHeightmaps>();
    app.insert_resource(ColumnScheduler {
        config: SchedulerConfig {
            max_in_flight: drive.in_flight,
            max_dispatch_per_tick: drive.batch,
            ..Default::default()
        },
        ..Default::default()
    });
    app.add_systems(
        Update,
        (
            enqueue_pending_columns,
            dispatch_column_generation,
            process_completed_columns,
            deliver_merged_columns,
        )
            .chain(),
    );

    let rungs = dim.ctx.rungs();
    let requests = ordered(wanted, drive.order);
    let mut sections: BTreeMap<ColumnPos, Vec<Entity>> = BTreeMap::new();
    let mut next = 0usize;
    let mut evicted = 0usize;
    let mut firings = 0usize;
    let deadline = Instant::now() + Duration::from_secs(600);

    loop {
        if next < requests.len() {
            let col = requests[next];
            next += 1;
            let ids = y_sections
                .iter()
                .map(|&y| {
                    request_section(
                        app.world_mut(),
                        dimension_entity,
                        SectionPos::new(col.x, y, col.z),
                    )
                })
                .collect();
            sections.insert(col, ids);
        }

        app.update();

        match drive.evict {
            Evict::OnTicks if firings < EVICT_FIRINGS => {
                let n = force_eviction(&mut app, wanted, drive.evict, false, rungs);
                if n > 0 {
                    firings += 1;
                    evicted += n;
                }
            }
            Evict::UnderARun if firings < 2 => {
                let n = force_eviction(&mut app, wanted, drive.evict, firings == 1, rungs);
                if n > 0 {
                    firings += 1;
                    evicted += n;
                }
            }
            _ => {}
        }

        let delivered = next == requests.len()
            && sections.values().flatten().all(|entity| {
                app.world().get::<SectionStage>(*entity) == Some(&SectionStage::Loaded)
            });
        if delivered {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{drive:?} never delivered the region"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    // What a column was delivered with is what it stays. The merged blocks
    // outlive the delivery — a section ticketed later is delivered from them —
    // so a re-merge after an eviction has to land on the same column.
    let scheduler = app.world().resource::<ColumnScheduler>();
    let standing: Vec<(ColumnPos, Column)> = wanted
        .iter()
        .filter_map(|col| {
            let merged = scheduler.store.merged(*col)?;
            Some((*col, decode(&merged.sections)))
        })
        .collect();

    let region: Region = sections
        .into_iter()
        .map(|(col, entities)| {
            let column = entities
                .iter()
                .map(|entity| {
                    let blocks = app
                        .world()
                        .get::<ChunkBlocks>(*entity)
                        .unwrap_or_else(|| panic!("{col:?} was delivered without its blocks"));
                    (0..ColumnBlocks::SECTION_VOLUME)
                        .map(|index| blocks.get_cell(index & 15, index >> 8, (index >> 4) & 15))
                        .collect()
                })
                .collect();
            (col, column)
        })
        .collect();

    for (col, merged) in standing {
        let delivered = region.get(&col).expect("every wanted column was delivered");
        assert_eq!(
            merged.len(),
            delivered.len(),
            "{drive:?}: the merged {col:?} carries a different number of sections"
        );
        for (slot, (merged, delivered)) in merged.iter().zip(delivered.iter()).enumerate() {
            assert!(
                merged == delivered,
                "{drive:?}: the merged {col:?} left section slot {slot} \
                 unlike the one it delivered"
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
        for (target, delta) in run_region(ctx, &snapshots, 0) {
            if delta.writes.is_empty() {
                continue;
            }
            carried += 1;
            crossed += usize::from(target != col);
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

/// The nine columns around the origin, through every worker count, request
/// order, batch size and eviction the definition leaves free, for each program
/// `Run` has.
#[test]
fn the_parallel_ladder_delivers_the_oracle_region() {
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
    ] {
        assert_region_agrees(consumer, 1, &small_drives());
    }
}

/// The same comparison over forty-nine wanted columns — a hundred and
/// twenty-one fills per run, so the interior columns are further from the edge
/// of the region than any dependency reaches.
#[test]
fn the_parallel_ladder_delivers_the_large_oracle_region() {
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

/// A column's block entities go to the dimension a live section names, and the
/// first-enqueued section is not necessarily live: a single section can be
/// unloaded on its own while the rest of the column stands, and the pending
/// entry keeps it at the head. Resolving the dimension from that one alone drops
/// every nest the column grew.
#[test]
fn a_dead_first_section_does_not_take_the_column_s_block_entities_with_it() {
    use crate::world::block_entity::BlockEntity;
    use crate::world::chunk::{ColumnKey, PendingColumn};
    use mcrs_minecraft_level::world::dimension::InDimension;
    use mcrs_minecraft_worldgen_feature_place::block_entity::{BeeOccupant, GeneratedBlockEntity};

    let mut app = App::new();
    app.add_message::<SectionStageChanged>();
    app.init_resource::<PendingColumnHeightmaps>();
    app.insert_resource(ColumnScheduler::default());

    let dim = app.world_mut().spawn_empty().id();
    let dead = app.world_mut().spawn(InDimension(dim)).id();
    let live = app.world_mut().spawn(InDimension(dim)).id();
    app.world_mut().entity_mut(dead).despawn();

    let col = ColumnPos::new(4, 9);
    {
        let mut scheduler = app.world_mut().resource_mut::<ColumnScheduler>();
        scheduler.pending.insert(
            ColumnKey::new(0, col),
            PendingColumn::new(vec![(dead, 0), (live, 1)]),
        );
        scheduler.store.insert_staged(
            col,
            0,
            Arc::new(FilledSnapshot {
                col,
                y_sections: Arc::from(vec![0, 1].as_slice()),
                sections: vec![None, None],
                terrain: None,
                maps: None,
                source: mcrs_minecraft_worldgen_generator::task::ColumnSource::Generated,
                block_entities: vec![GeneratedBlockEntity::Beehive {
                    x: 70,
                    y: 20,
                    z: 150,
                    bees: vec![BeeOccupant::bee(11)],
                }],
                entities: Vec::new(),
            }),
        );
        scheduler.store.set_stage(col, Stage::Merged(0));
    }

    app.add_systems(Update, deliver_merged_columns);
    app.update();

    let nests = app
        .world_mut()
        .query::<&BlockEntity>()
        .iter(app.world())
        .filter(|held| matches!(held.0, GeneratedBlockEntity::Beehive { .. }))
        .count();
    assert_eq!(nests, 1, "the delivered column's nest was dropped");
}

/// A delivery owes only the sections it was asked for — the ticket layer caps
/// how many it spawns a tick and cuts a column's sections across two of them —
/// while the fill covers the whole dimension, because the bedrock floor is a
/// material rule and a column surfaced over a slice would be left open at the
/// bottom.
#[test]
fn a_delivery_carrying_part_of_a_column_still_lays_its_bedrock_floor() {
    use bevy_app::App;
    use bevy_ecs::entity::Entity;
    use mcrs_minecraft_biome::source::BiomeSource;
    use mcrs_minecraft_core::SectionPos;
    use mcrs_minecraft_protocol::ColumnPos;

    use crate::world::chunk::carried_sections;
    use mcrs_minecraft_worldgen_generator::stages::fill_column;
    use mcrs_minecraft_worldgen_generator::task::CancellationToken;

    let (registry, ids) = overworld_biome_registry();
    let (router, material) = overworld_material_router(2, &ids);
    let ctx = surface_fill_context(
        router,
        material,
        registry,
        BiomeSource::MultiNoise(MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
            biomes: None,
        }),
    );

    let col = ColumnPos::new(3, -7);
    let carried = [-4, 3, 4];
    let mut app = App::new();
    let sections: Vec<(Entity, SectionPos)> = carried
        .iter()
        .map(|&y| {
            (
                app.world_mut().spawn_empty().id(),
                SectionPos::new(col.x, y, col.z),
            )
        })
        .collect();

    let y_sections = ctx.y_sections.clone();
    let mut column = ColumnBlocks::new(&y_sections);
    let snapshot = fill_column(&ctx, col, &mut column, &CancellationToken::new())
        .expect("the fill was not cancelled");

    let bottom = y_sections[0];
    let delivered = carried_sections(&sections, &snapshot.sections, bottom);
    assert_eq!(
        delivered.len(),
        carried.len(),
        "the delivery returned sections it was not asked for"
    );
    let bedrock = VoxelId::from(corpus().default_state("minecraft:bedrock"));
    let floor = delivered
        .iter()
        .find(|(_, pos, _)| pos.y == carried[0])
        .expect("the bottom section came back");
    let (palette, _) = floor.2.as_ref().expect("the bottom section carries blocks");
    let mut states = Vec::with_capacity(ColumnBlocks::SECTION_VOLUME);
    palette.0.for_each(|state| states.push(state));
    // The palette runs y, then z, then x, so the first layer is the floor of
    // the dimension, which `bedrock_floor` covers whole.
    assert!(
        states[..256].iter().all(|state| *state == bedrock),
        "the bottom of the world is open: the material rules never ran over this dispatch"
    );
}
