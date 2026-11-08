//! Random ticks of one step over the overworld of a real save, on one core.
//!
//! `MCRS_BENCH_WORLD` names a world directory of the current save format; without it the bench
//! prints why and measures nothing. Every full column of the save is loaded into the section
//! index of a dimension, with an entity for each section, and random-ticked through the volume
//! of a cell, as the engine does: the step draws the random picks of the sections of the random
//! tick index, then runs the precipitation picks and the hits. The states of the block types of
//! SIM-MODEL-009 Table 1 tick randomly. One handler reads one block, the other the footprint of
//! its row; neither writes. The bench prints the handler runs of a step, and asserts that no pick
//! runs again.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a bench reports a failure by panicking"
)]

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::prelude::{Entity, Query, With, World};
use bevy_ecs::system::SystemState;
use criterion::{Criterion, criterion_group, criterion_main};
use mcrs_minecraft_anvil::{Chunk, ChunkStatus, PaletteLookup, Properties, RegionFile};
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_engine::CellVolume;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, section_states};
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::BlockTicking;
use mcrs_minecraft_engine_world::world::storage::random_ticks::{RandomListing, RandomTickIndex};
use mcrs_minecraft_engine_world::world::storage::section::{SectionBundle, SectionIndex};
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockShard, Cx, Env, Footprints, GameTick, KernelConfig, Kinds,
    PartitionKind, RandomPicks, RandomTickTable, RandomTicks, SemanticOrder, Serial, ShardId,
    SimulationVolume, StepOutput, StreamSeed, random_step,
};
use mcrs_minecraft_worldgen_generator::saved::CorpusBlockStates;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

/// The default of the game rule `random_tick_speed`.
const SPEED: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bench;

impl Kinds for Bench {
    type Block = u16;
    type Fluid = u16;
}

/// The box that a random tick of a state reads (Table 1): `±dx` on both horizontal axes,
/// `low..=high` on the vertical axis, and, for the rows with `n`, the run of the same block below.
#[derive(Clone, Copy)]
struct Footprint {
    dx: i32,
    low: i32,
    high: i32,
    run_below: bool,
    /// Lava takes two fluid random ticks for each pick (SIM-RND-002).
    calls: u8,
}

const fn bounded(dx: i32, low: i32, high: i32) -> Footprint {
    Footprint {
        dx,
        low,
        high,
        run_below: false,
        calls: 1,
    }
}

const fn run(dx: i32, high: i32) -> Footprint {
    Footprint {
        run_below: true,
        ..bounded(dx, 0, high)
    }
}

fn flag(block: &BlockEntry, state: BlockStateId, property: &str) -> Option<bool> {
    block
        .value_of(state, property)
        .map(|value| value.renders_to("true"))
}

fn ages(definitions: &BlockDefinitions, name: &str) -> bool {
    if name.starts_with("waxed_") || name.starts_with("oxidized_") {
        return false;
    }
    let base = name
        .strip_prefix("exposed_")
        .or_else(|| name.strip_prefix("weathered_"))
        .unwrap_or(name);
    let base = if base == "copper_block" {
        "copper"
    } else {
        base
    };
    definitions
        .block(&format!("minecraft:oxidized_{base}"))
        .is_some()
}

/// The Table 1 row of `state`, or `None` when the state does not tick randomly.
fn footprint(
    definitions: &BlockDefinitions,
    block: &BlockEntry,
    state: BlockStateId,
) -> Option<Footprint> {
    let name = block.identifier.path();
    Some(match name {
        "grass_block" | "mycelium" => bounded(1, -3, 2),
        "crimson_nylium" | "warped_nylium" => bounded(0, 0, 1),
        "wheat" | "carrots" | "potatoes" | "beetroots" | "torchflower_crop" | "pumpkin_stem"
        | "melon_stem" => bounded(1, -1, 0),
        "pitcher_crop" => bounded(1, -1, 1),
        "mangrove_propagule" if flag(block, state, "hanging") == Some(true) => bounded(0, 0, 1),
        "mangrove_propagule" => bounded(10, -13, 34),
        "bamboo_sapling" => bounded(0, 0, 1),
        _ if name.ends_with("_sapling") => bounded(10, -13, 34),
        "cocoa" | "nether_wart" | "sweet_berry_bush" => bounded(0, 0, 1),
        "brown_mushroom" | "red_mushroom" => bounded(5, -6, 5),
        _ if name.ends_with("_leaves") => {
            if flag(block, state, "persistent") != Some(false) {
                return None;
            }
            bounded(0, 0, 0)
        }
        "ice" | "snow" => bounded(0, 0, 0),
        "farmland" => bounded(4, 0, 1),
        "sugar_cane" | "nether_portal" => run(0, 1),
        "cactus" => run(1, 1),
        "bamboo" => bounded(0, -16, 1),
        "vine" => bounded(4, -1, 1),
        "kelp" | "twisting_vines" => bounded(0, 0, 1),
        "weeping_vines" | "cave_vines" | "turtle_egg" => bounded(0, -1, 0),
        "chorus_flower" => bounded(2, -5, 2),
        "pointed_dripstone" => bounded(0, -22, 12),
        "icicle" => bounded(0, -15, 2),
        "sulfur_spike" => bounded(0, -13, 2),
        "budding_amethyst" => bounded(1, -1, 1),
        "redstone_ore" | "deepslate_redstone_ore" => {
            if flag(block, state, "lit") != Some(true) {
                return None;
            }
            bounded(0, 0, 0)
        }
        "open_eyeblossom" | "closed_eyeblossom" => bounded(3, -2, 2),
        "potted_open_eyeblossom" | "potted_closed_eyeblossom" | "dried_ghast" | "firefly_bush" => {
            bounded(0, 0, 0)
        }
        "lava" => Footprint {
            calls: 2,
            ..bounded(3, 0, 3)
        },
        _ if ages(definitions, name) => bounded(4, -4, 4),
        _ => return None,
    })
}

struct Facts;

impl BlockFacts<Bench> for Facts {
    fn block_of(&self, state: BlockStateId) -> u16 {
        state.index() as u16
    }

    fn fluid_of(&self, _: BlockStateId) -> Option<u16> {
        None
    }

    fn void_air(&self) -> BlockStateId {
        BlockStateId::AIR
    }

    fn is_air(&self, state: BlockStateId) -> bool {
        state == BlockStateId::AIR
    }

    fn is_rail(&self, _: BlockStateId) -> bool {
        false
    }

    fn is_redstone_wire(&self, _: BlockStateId) -> bool {
        false
    }

    fn legacy_block_of_fluid(&self, state: BlockStateId) -> BlockStateId {
        state
    }
}

/// What a handler reads: the floor of a handler, or the whole footprint of its row.
#[derive(Clone, Copy, Debug)]
enum Handler {
    OneRead,
    FullBox,
}

struct Reads<'r> {
    rows: &'r [Option<Footprint>],
    handler: Handler,
    runs: AtomicU64,
}

impl BlockBehaviour<Bench> for Reads<'_> {
    fn random_ticks(&self, state: BlockStateId) -> RandomTicks {
        match self.rows.get(state.index()) {
            Some(Some(_)) => RandomTicks::BLOCK,
            _ => RandomTicks::empty(),
        }
    }

    fn reach(&self, state: BlockStateId) -> u8 {
        reach(self.rows, state)
    }

    fn random_tick<V: SimulationVolume, F: BlockFacts<Bench>>(
        &self,
        cx: &mut Cx<'_, Bench, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _: &mut LegacyRandom,
    ) {
        let Some(Some(row)) = self.rows.get(state.index()).copied() else {
            return;
        };
        self.runs.fetch_add(1, Ordering::Relaxed);
        if let Handler::OneRead = self.handler {
            black_box(cx.state(BlockPos::new(at.x, at.y + 1, at.z)).ok());
            return;
        }
        for _ in 0..row.calls {
            for dx in -row.dx..=row.dx {
                for dy in row.low..=row.high {
                    for dz in -row.dx..=row.dx {
                        black_box(
                            cx.state(BlockPos::new(at.x + dx, at.y + dy, at.z + dz))
                                .ok(),
                        );
                    }
                }
            }
            let mut y = at.y - 1;
            while row.run_below && cx.state(BlockPos::new(at.x, y, at.z)) == Ok(state) {
                y -= 1;
            }
        }
    }
}

struct AnyBiome;

impl PaletteLookup<u8> for AnyBiome {
    fn resolve(&self, _: &str, _: Properties<'_>) -> Option<u8> {
        Some(0)
    }
}

fn region_dir(world: &Path) -> PathBuf {
    let current = world.join("dimensions/minecraft/overworld/region");
    if current.is_dir() {
        current
    } else {
        world.join("region")
    }
}

/// A full column, or one that an upgrade marked to run only its biome steps again on the way
/// back to full: its blocks are those of a full column.
fn blocks_complete(chunk: &Chunk) -> bool {
    chunk.status == ChunkStatus::Full
        || chunk.retrogen.as_ref().is_some_and(|retrogen| {
            retrogen.target_status == ChunkStatus::Full
                && !retrogen.has_below_zero_retrogen
                && retrogen
                    .statuses_to_rerun
                    .iter()
                    .all(|status| matches!(status, ChunkStatus::Biomes | ChunkStatus::NoiseBiomes))
        })
}

/// The full columns of the overworld of a save, as the engine holds them: the blocks and the
/// entity of each section in the section index of the dimension.
struct Loaded {
    world: World,
    dimension: Entity,
}

fn load(world: &Path, definitions: &BlockDefinitions, table: &RandomTickTable) -> Loaded {
    let lookup = CorpusBlockStates(definitions);
    let state_count = definitions.state_count();
    let mut ecs = World::new();
    let dimension = ecs.spawn_empty().id();
    let mut section_index = SectionIndex::new();
    let mut index = RandomTickIndex::new(table.clone());
    let mut sections = 0;
    let (mut columns, mut failed) = (0, 0);
    let mut first_error = None;
    let dir = region_dir(world);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the world holds a region directory of the overworld")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "mca"))
        .collect();
    files.sort();
    for path in files {
        let Ok(region) = RegionFile::open(&path) else {
            continue;
        };
        for pos in region.present().collect::<Vec<_>>() {
            match region.read_chunk(pos, &lookup, &AnyBiome) {
                Ok(Some(chunk)) if blocks_complete(&chunk) => {
                    columns += 1;
                    for section in chunk.sections {
                        let Some(blocks) = section.block_states else {
                            continue;
                        };
                        let states = section_states(&VoxelPalette(blocks), state_count);
                        let at = SectionPos::new(pos.x, i32::from(section.y), pos.z);
                        let blocks = ChunkBlocks::new(states);
                        let entity = ecs
                            .spawn((SectionBundle::new(InDimension(dimension), at), BlockTicking))
                            .id();
                        section_index.insert(at, entity);
                        section_index.land(at, entity, blocks, Some(&mut index), true);
                        sections += 1;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    failed += 1;
                    first_error.get_or_insert_with(|| error.to_string());
                }
            }
        }
    }
    println!(
        "{}: {columns} columns with complete blocks, {} sections, {failed} columns failed{}",
        world.display(),
        sections,
        first_error.map_or(String::new(), |error| format!(" (first: {error})")),
    );
    ecs.entity_mut(dimension)
        .insert((section_index, DimensionTypeConfig::new(-64, 384)));
    ecs.insert_resource(index);
    Loaded {
        world: ecs,
        dimension,
    }
}

fn reach(rows: &[Option<Footprint>], state: BlockStateId) -> u8 {
    rows.get(state.index())
        .copied()
        .flatten()
        .map_or(Footprints::DEFAULT, |row| row.dx as u8)
}

/// The Table 1 row of each state.
fn rows(definitions: &BlockDefinitions) -> Vec<Option<Footprint>> {
    let mut rows = vec![None; definitions.state_count()];
    for block in definitions.blocks() {
        let base = usize::from(block.base_state_id.number());
        for offset in 0..usize::from(block.state_count) {
            let state = BlockStateId::checked((base + offset) as u16, definitions.state_count())
                .expect("a state of the block");
            rows[state.index()] = footprint(definitions, block, state);
        }
    }
    rows
}

fn random_ticks_on_a_save(c: &mut Criterion) {
    #[expect(
        clippy::disallowed_methods,
        reason = "a bench takes the path of its input from the environment"
    )]
    let world = std::env::var_os("MCRS_BENCH_WORLD");
    let Some(world) = world else {
        println!(
            "skipped: MCRS_BENCH_WORLD names no world directory (a save upgraded to the current save format)"
        );
        return;
    };
    let definitions = &test_corpus().0.0;
    let rows = rows(definitions);
    let states = || definitions.states();
    let table = RandomTickTable::new(states(), |state| match rows[state.index()] {
        Some(_) => RandomTicks::BLOCK,
        None => RandomTicks::empty(),
    });
    let Loaded {
        world: mut ecs,
        dimension,
    } = load(Path::new(&world), definitions, &table);
    let mut state: SystemState<(WorldViewMut, Query<(), With<BlockTicking>>)> =
        SystemState::new(&mut ecs);
    let (mut blocks, ticking) = state.get_mut(&mut ecs).expect("valid params");
    let mut volume = CellVolume {
        view: blocks.get_mut(dimension).expect("a dimension"),
        owned: None,
        ticking: &ticking,
    };
    let config = KernelConfig::default();
    let seed = StreamSeed::new(4, "minecraft:overworld");
    let footprints = Footprints::new(states(), |state| reach(&rows, state));
    let mut listing = RandomListing::default();
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    let mut now = 0;
    for handler in [Handler::OneRead, Handler::FullBox] {
        let behaviour = Reads {
            rows: &rows,
            handler,
            runs: AtomicU64::new(0),
        };
        let env = Env {
            partition: ShardId(0),
            facts: &Facts,
            behaviour: &behaviour,
            policy: &SemanticOrder,
            kind: PartitionKind::Shard,
            config: &config,
        };
        let steps = now;
        c.bench_function(&format!("random step on a save, {handler:?}"), |b| {
            b.iter(|| {
                now += 1;
                out.clear();
                volume
                    .view
                    .random_ticks_mut()
                    .expect("the world holds a random tick index")
                    .draw(|_| true, SPEED, seed.sections(GameTick(now)), &mut listing);
                let picks = RandomPicks {
                    columns: &listing.columns,
                    hits: &listing.hits,
                    speed: SPEED,
                    table: &table,
                    footprints: &footprints,
                    seed,
                };
                random_step(
                    &mut shard,
                    GameTick(now),
                    &mut volume,
                    &env,
                    picks,
                    &mut out,
                    &Serial,
                );
                assert_eq!(
                    out.report.random_reruns, 0,
                    "a read-only handler of Table 1 fits"
                );
            });
        });
        let runs = behaviour.runs.load(Ordering::Relaxed) as f64;
        println!(
            "{handler:?}: {:.0} handler runs in each step, {} precipitation columns",
            runs / (now - steps).max(1) as f64,
            listing.columns.len(),
        );
    }
}

criterion_group!(benches, random_ticks_on_a_save);
criterion_main!(benches);
