//! The single-threaded oracle against the parallel scheduler.
//!
//! The definition of the stage leaves the scheduler free in which worker runs a
//! unit, when, in what order relative to units it shares no position with, and
//! whether a unit ran once or was re-run after an eviction. Every one of those
//! degrees of freedom is driven here, and every one must leave the decoded
//! blocks identical to the oracle's.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::{App, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::IVec3;
use bevy_tasks::TaskPoolBuilder;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::lifecycle::markers::{ChunkLoaded, ChunkLoading};
use mcrs_minecraft_protocol::ColumnPos;

use crate::world::chunk::{
    CHUNK_TASK_POOL, CancellationToken, ColumnScheduler, SchedulerConfig, deliver_merged_columns,
    dispatch_column_generation, enqueue_pending_columns, process_completed_columns,
};
use crate::world::format::anvil::SectionData;
use crate::world::generate::modern_carvers::ModernCarverBlockIds;
use crate::world::generate::stages::{
    ColumnGenerator, ColumnProgram, FillContext, dimension_y_sections, fill_pooled, run_region,
};
use crate::world::generate::staging::{FilledSnapshot, RegionSnapshots, Stage, region_column};
use crate::world::generate::structures::index::{BiomeLookup, StructureIndex};
use crate::world::generate::structures::live_sets;
use crate::world::generate::{BetaCaveBlockIds, ColumnBlocks, SurfaceIds};
use crate::world::heightmap::{PendingColumnHeightmaps, heightmap_predicates};
use mcrs_minecraft_worldgen::structure::frozen::DimensionStructureTables;

use super::corpus_ores::{one_biome_registry, ore_program, ore_tables};
use super::structures::frozen_shared;
use super::{
    biome_index, block_tags, blocks, build_beta_router, build_program_with, corpus_features,
    generate_region, one_step,
};

/// One decoded column: its sections, each a flat block array in the packed
/// order of the palettes themselves.
type Column = Vec<Vec<VoxelId>>;
type Region = BTreeMap<ColumnPos, Column>;

/// Which program `Run` executes. Neither is the scheduler's business — it is
/// the same three stages either way — but a consumer that writes nothing would
/// leave the whole comparison two empty pipelines against each other, which is
/// what this file compared before either landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Consumer {
    /// Beta's populate step, which is `Run` for a beta dimension.
    BetaOre,
    /// The modifier machine over the corpus's own ore features.
    ModernOre,
    /// The corpus's own forest trees, whose crowns and decorators reach
    /// several blocks past the column that seeds them — the widest footprint
    /// any consumer has, and so the one the merge is really tested by.
    Tree,
    /// Every placed feature the corpus declares, over one biome: lakes, geodes,
    /// icebergs, dripstone clusters and huge fungi all writing into the same
    /// nine columns, which is a far wider and more varied set of deltas than
    /// any single family produces.
    Corpus,
    /// A plains village over the region's centre: jigsaw pieces that straddle
    /// columns, each column reading the whole start and writing the pieces
    /// that cross it, block entities included.
    Village,
    /// A pillager outpost, the other jigsaw structure a plains column starts.
    Outpost,
}

/// The dimension the tree consumer runs in: the overworld router, forest
/// everywhere, and the biome's own `trees_birch_and_oak_leaf_litter`.
const TREE_BIOME: &str = "minecraft:forest";
const TREE_FEATURE: &str = "minecraft:trees_birch_and_oak_leaf_litter";
const TREE_SEED: u64 = 4242;

/// The dimension the corpus consumer runs in.
const CORPUS_BIOME: &str = "minecraft:plains";
const CORPUS_SEED: u64 = 0xC0FFEE;

/// The dimension the two structure consumers run in: no features, so every
/// write is a structure's.
const VILLAGE_BIOME: &str = "minecraft:plains";
const VILLAGE_SEED: u64 = 0x51A6E;

/// One dimension, described once: the context the oracle drives the three stage
/// functions with, and the resources the dispatcher rebuilds that very context
/// from. The two must agree, or the comparison below is between two different
/// worlds rather than two orderings of one.
pub(super) struct Dimension {
    pub(super) ctx: FillContext,
    registry: Arc<RegistrySnapshot<Biome>>,
    /// The column the compared region is centred on.
    pub(super) centre: ColumnPos,
}

impl Dimension {
    fn install(&self, app: &mut App) {
        app.insert_resource(self.ctx.clone());
        app.insert_resource(blocks().clone());
        app.insert_resource(RegistrySnapshot::clone(&self.registry));
        app.insert_resource(block_tags().clone());
        app.insert_resource(
            self.ctx
                .predicates
                .clone()
                .expect("the dimension carries the heightmap table"),
        );
    }
}

/// The overworld with one fixed biome and the shipped structure sets, centred
/// on the nearest start of `structure` the index finds from the origin.
pub(super) fn structure_dimension(structure: &str) -> Dimension {
    let frozen = frozen_shared();
    let seed = VILLAGE_SEED;
    let (mut ctx, _) = super::trees::dimension_with(
        VILLAGE_BIOME,
        |registry| {
            build_program_with(
                &one_step(vec![], VILLAGE_BIOME),
                corpus_features(),
                registry,
                seed as i64,
                Some(frozen),
            )
        },
        seed,
    );
    let biome = biome_index()
        .get(VILLAGE_BIOME)
        .expect("the biome index holds the corpus");
    let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
    mask.insert(biome as usize);
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask),
    };
    let index = StructureIndex::new(
        Arc::new(tables),
        seed as i64,
        Arc::clone(&ctx.router),
        BiomeLookup::Fixed(biome),
        ctx.predicates.clone(),
        -64,
        384,
    );
    let wanted = frozen.structure_ids[&ResourceLocation::parse(structure).unwrap()];
    let (pos, _) = index
        .locate(IVec3::ZERO, &[wanted])
        .unwrap_or_else(|| panic!("no {structure} within the search radius"));
    let centre = ColumnPos::new(pos.x >> 4, pos.z >> 4);
    assert!(
        !index.starts_reaching(centre).is_empty(),
        "{structure} at {centre:?} does not reach its own column"
    );
    ctx.structures = Some(Arc::new(index));
    let (_, registry) = ctx.biome.clone().expect("the dimension has a biome");
    Dimension {
        ctx,
        registry,
        centre,
    }
}

fn fill_context(consumer: Consumer) -> Dimension {
    match consumer {
        Consumer::Village => return structure_dimension("minecraft:village_plains"),
        Consumer::Outpost => return structure_dimension("minecraft:pillager_outpost"),
        _ => {}
    }
    if let Consumer::Tree | Consumer::Corpus = consumer {
        let (ctx, _) = if consumer == Consumer::Tree {
            super::trees::tree_dimension(TREE_BIOME, TREE_FEATURE, TREE_SEED)
        } else {
            let tables = Arc::new(super::corpus_generators::every_placed_feature());
            super::trees::dimension_over(CORPUS_BIOME, tables, CORPUS_SEED)
        };
        let (_, registry) = ctx.biome.clone().expect("the dimension has a biome");
        return Dimension {
            ctx,
            registry,
            centre: ColumnPos::new(0, 0),
        };
    }
    let router = Arc::new(build_beta_router());
    let y_sections = dimension_y_sections(&router, -64, 24);
    let (source, registry, tables) = match consumer {
        Consumer::BetaOre => {
            let (source, registry) = super::beta_surface::build_beta_biome_source();
            (Some(Arc::new(source)), Arc::new(registry), None)
        }
        Consumer::ModernOre => {
            let (tables, _) = ore_tables();
            (None, Arc::new(one_biome_registry()), Some(Arc::new(tables)))
        }
        Consumer::Tree | Consumer::Corpus | Consumer::Village | Consumer::Outpost => {
            unreachable!("the feature and structure dimensions returned above")
        }
    };
    let program = match consumer {
        Consumer::BetaOre => ColumnProgram {
            generator: ColumnGenerator::Beta(Arc::new(BetaCaveBlockIds::resolve(&blocks().0))),
            carvers: source
                .as_deref()
                .map(|source| Arc::new(super::beta_carver_table(source))),
            features: Some(Arc::new(super::beta_populate_program(
                &registry,
                router.world_seed as i64,
            ))),
        },
        _ => ColumnProgram {
            generator: ColumnGenerator::Modern {
                multi_noise: None,
                // Derived from the registry alone, exactly as the dispatcher derives
                // it. Neither consumer reaches the material surface — it needs a
                // biome grid, which only a multi-noise or fixed source builds — but
                // the context has to match all the same.
                surface: Some(Arc::new(SurfaceIds::resolve(&blocks().0, &registry))),
                carver_blocks: Arc::new(ModernCarverBlockIds::resolve(
                    &blocks().0,
                    Some(block_tags()),
                )),
            },
            carvers: None,
            features: tables.as_ref().map(|tables| Arc::new(ore_program(tables))),
        },
    };
    let ctx = FillContext {
        blocks: blocks().0.clone(),
        biome: source.clone().map(|src| (src, registry.clone())),
        // The modern vein probes `OCEAN_FLOOR_WG`, which is a terrain map, so
        // without the table it would place nothing at all.
        predicates: Some(heightmap_predicates(blocks(), block_tags())),
        saved: None,
        program,
        router,
        y_sections: y_sections.clone(),
        structures: None,
    };
    Dimension {
        ctx,
        registry,
        centre: ColumnPos::new(0, 0),
    }
}

fn decode(sections: &[Option<SectionData>]) -> Column {
    sections
        .iter()
        .map(|section| {
            let mut cells = vec![VoxelId::default(); ColumnBlocks::SECTION_VOLUME];
            if let Some((palette, _)) = section {
                for (index, cell) in cells.iter_mut().enumerate() {
                    *cell = palette.get_cell(index & 15, index >> 8, (index >> 4) & 15);
                }
            }
            cells
        })
        .collect()
}

pub(super) fn region_columns(radius: i32) -> Vec<ColumnPos> {
    (-radius..=radius)
        .flat_map(|x| (-radius..=radius).map(move |z| ColumnPos::new(x, z)))
        .collect()
}

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
                    app.world_mut()
                        .spawn((SectionPos::new(col.x, y, col.z), ChunkLoading))
                        .id()
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
            && sections
                .values()
                .flatten()
                .all(|entity| app.world().get::<ChunkLoaded>(*entity).is_some());
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
    if consumer != Consumer::Outpost {
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
        Consumer::Village,
        Consumer::Outpost,
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
        Consumer::Village,
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
    use mcrs_minecraft_decoration::block_entity::{BeeOccupant, GeneratedBlockEntity};
    use mcrs_minecraft_level::world::dimension::InDimension;

    let mut app = App::new();
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
                source: crate::world::chunk::ColumnSource::Generated,
                block_entities: vec![GeneratedBlockEntity::Beehive {
                    x: 70,
                    y: 20,
                    z: 150,
                    bees: vec![BeeOccupant::bee(11)],
                }],
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
