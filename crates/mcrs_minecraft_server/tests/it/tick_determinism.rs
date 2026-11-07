//! The world a tick produces must not depend on where the worlds tick or on the order in which
//! the generation and light steps ended. The world has two cells, so with threads the cells tick
//! at the same time and exchange the columns at their boundary.
//!
//! Each run ticks in lockstep: after every tick it waits until the generation and light steps
//! that the tick started have ended, without applying their writes. The writes then reach every
//! run at the same tick, so any difference between runs comes from the threads.

use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_engine::muxer::tile_delta_log::TileDeltaLog;
use mcrs_minecraft_engine_server::Placement;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::block_entity::BlockEntity;
use mcrs_minecraft_engine_world::world::storage::column::ColumnPosComponent;
use mcrs_minecraft_heightmap::{MotionHeightmap, SurfaceHeightmap};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

use crate::host_app::label_of;
use crate::world_server::{
    OVERWORLD, WorldServer, block_state, cell_worlds, dimension_of, lockstep_tick, spawn_viewer,
    wait_for_generation,
};

const TICKS: usize = 60;

struct Scenario {
    dimension: Entity,
    player: Entity,
    glowstone: BlockStateId,
    stone: BlockStateId,
}

impl Scenario {
    fn place(&self, world: &mut World, pos: BlockPos, new_state: BlockStateId) {
        world
            .resource_mut::<Messages<BlockSetRequest>>()
            .write(BlockSetRequest {
                dimension: self.dimension,
                pos,
                new_state,
                flags: BlockUpdateFlags::all(),
                recursion_left: 512,
            });
    }

    /// The input of tick `at`. The first ticks load the view and light it.
    fn script(&self, world: &mut World, at: usize) {
        match at {
            30 => {
                for x in 0..4 {
                    self.place(world, BlockPos::new(x * 5, 90, 3), self.glowstone);
                }
                self.place(world, BlockPos::new(8, 95, 8), self.stone);
            }
            34 => {
                let mut transform = world
                    .get_mut::<Transform>(self.player)
                    .expect("the player stands somewhere");
                transform.translation.x += 48.0;
            }
            45 => {
                self.place(world, BlockPos::new(5, 90, 3), BlockStateId::AIR);
                for z in 0..3 {
                    self.place(world, BlockPos::new(56, 92, z * 7), self.glowstone);
                }
            }
            _ => {}
        }
    }
}

/// Feeds `value`'s `Debug` form to `hasher` without building the string.
struct HashWriter<'a>(&'a mut DefaultHasher);

impl fmt::Write for HashWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0.write(text.as_bytes());
        Ok(())
    }
}

fn hash_debug(hasher: &mut DefaultHasher, value: &impl fmt::Debug) {
    fmt::write(&mut HashWriter(hasher), format_args!("{value:?}")).expect("hashing never fails");
}

/// Hashes the light values, so a dense section that holds one value hashes as that uniform value.
fn hash_light(hasher: &mut DefaultHasher, light: Option<&LightStorage>) {
    match light {
        None => 0u8.hash(hasher),
        Some(LightStorage::Empty) => (1u8, 0u8).hash(hasher),
        Some(LightStorage::Uniform(value)) => (1u8, *value).hash(hasher),
        Some(LightStorage::Dense(packed)) => {
            let bytes = &packed.0[..];
            let first = bytes[0];
            if first >> 4 == first & 0x0F && bytes.iter().all(|&byte| byte == first) {
                (1u8, first & 0x0F).hash(hasher);
            } else {
                2u8.hash(hasher);
                bytes.hash(hasher);
            }
        }
    }
}

/// The simulation-visible state of the dimension: every section with its stage, light and, as
/// `hashed` asks, its blocks; every column with its heightmaps; the block entities and the
/// players. Entities enter by id, so a different allocation order is a different world.
fn world_hash(world: &mut World, hashed: Hashed) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut sections: Vec<_> = world
        .query::<(
            Entity,
            &SectionPos,
            &SectionStage,
            Option<&ChunkBlocks>,
            Option<&BlockLight>,
            Option<&SkyLight>,
        )>()
        .iter(world)
        .collect();
    sections.sort_by_key(|(entity, pos, ..)| (pos.x, pos.y, pos.z, *entity));
    for (entity, pos, stage, chunk, block, sky) in sections {
        entity.hash(&mut hasher);
        pos.hash(&mut hasher);
        hash_debug(&mut hasher, stage);
        hash_light(&mut hasher, block.map(|light| &light.0));
        hash_light(&mut hasher, sky.map(|light| &light.0));
        if hashed == Hashed::WithBlocks {
            hash_debug(&mut hasher, &chunk.map(|chunk| &chunk.0));
        }
    }
    let mut columns: Vec<_> = world
        .query::<(
            Entity,
            &ColumnPosComponent,
            Option<&SurfaceHeightmap>,
            Option<&MotionHeightmap>,
        )>()
        .iter(world)
        .collect();
    columns.sort_by_key(|(entity, pos, ..)| (pos.0.x, pos.0.z, *entity));
    for (entity, pos, surface, motion) in columns {
        entity.hash(&mut hasher);
        pos.0.hash(&mut hasher);
        hash_debug(&mut hasher, &surface);
        hash_debug(&mut hasher, &motion);
    }
    let mut block_entities: Vec<_> = world
        .query::<(Entity, &BlockEntity)>()
        .iter(world)
        .collect();
    block_entities.sort_by_key(|(entity, _)| *entity);
    for (entity, block_entity) in block_entities {
        entity.hash(&mut hasher);
        hash_debug(&mut hasher, block_entity);
    }
    let mut players: Vec<_> = world
        .query_filtered::<(Entity, &Transform), With<Player>>()
        .iter(world)
        .collect();
    players.sort_by_key(|(entity, _)| *entity);
    for (entity, transform) in players {
        entity.hash(&mut hasher);
        hash_debug(&mut hasher, transform);
    }
    hasher.finish()
}

/// The hash of every cell of the dimension after each tick of the scenario, and of their blocks
/// after the last.
fn run(worlds: Placement) -> Vec<u64> {
    let mut app = WorldServer {
        worlds,
        cells: 2,
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let glowstone = block_state(&app, "minecraft:glowstone");
    let stone = block_state(&app, "minecraft:stone");
    // The first cell owns region 0, which starts at x = 0; the light of the blocks at x = 0
    // reaches the second cell.
    let world = first_cell(&mut app, label);
    let scenario = Scenario {
        dimension: dimension_of(world),
        player: spawn_viewer(world, PlayerSession(1), DVec3::new(8.5, 100.0, 8.5), 2),
        glowstone,
        stone,
    };

    app.init_resource::<TileDeltaLog>();
    let mut published_tiles = 0;
    let mut hashes = Vec::with_capacity(TICKS + 1);
    for at in 0..TICKS {
        scenario.script(first_cell(&mut app, label), at);
        lockstep_tick(&mut app, label);
        let mut hasher = DefaultHasher::new();
        for world in cell_worlds(&mut app, label) {
            world_hash(world, Hashed::WithoutBlocks).hash(&mut hasher);
        }
        // The tile deltas are a pure output of the tick, so the runs publish the same ones.
        let logged = std::mem::take(&mut app.world_mut().resource_mut::<TileDeltaLog>().0);
        for entry in logged.iter().filter(|entry| entry.dimension == label) {
            hash_debug(&mut hasher, &entry.deltas);
            published_tiles += entry.deltas.tiles.len();
        }
        hashes.push(hasher.finish());
    }
    let mut hasher = DefaultHasher::new();
    let mut loaded = Vec::new();
    for world in cell_worlds(&mut app, label) {
        world_hash(world, Hashed::WithBlocks).hash(&mut hasher);
        loaded.push(
            world
                .query::<&SectionStage>()
                .iter(world)
                .filter(|stage| **stage == SectionStage::Loaded)
                .count(),
        );
    }
    hashes.push(hasher.finish());
    assert!(
        loaded.iter().all(|&loaded| loaded > 0),
        "a cell generated nothing, so it compares nothing: {loaded:?}"
    );
    assert!(
        published_tiles > 0,
        "the player published no tile delta, so the deltas compare nothing"
    );
    hashes
}

fn first_cell(app: &mut App, label: Entity) -> &mut World {
    cell_worlds(app, label)
        .into_iter()
        .next()
        .expect("the world has a cell")
}

/// Whether a hash covers the blocks of every section. Light and stages cover each tick; the
/// blocks change only by generation and by the script, so the last tick covers them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hashed {
    WithBlocks,
    WithoutBlocks,
}

fn first_divergence(left: &[u64], right: &[u64]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
}

#[test]
fn the_world_a_tick_produces_does_not_depend_on_the_threads() {
    let inline = run(Placement::Inline);
    let threads = run(Placement::Thread);
    assert_eq!(
        first_divergence(&inline, &threads),
        None,
        "the threads diverge from Inline at the tick shown (the last entry holds the blocks)"
    );
}
