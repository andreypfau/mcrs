//! The world a tick produces must not depend on where the worlds tick or on the order in which
//! the generation and light steps ended.
//!
//! Each run ticks in lockstep: after every tick it waits until the generation and light steps
//! that the tick started have ended, without applying their writes. The writes then reach every
//! run at the same tick, so any difference between runs comes from the threads.

use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_tasks::block_on;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_engine::column_view::ColumnView;
use mcrs_minecraft_engine::muxer::tile_delta_log::TileDeltaLog;
use mcrs_minecraft_engine_server::{Placement, runner};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::block_entity::BlockEntity;
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_minecraft_heightmap::{MotionHeightmap, SurfaceHeightmap};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_light::LightLink;
use mcrs_minecraft_vanilla_worldgen::GenerationLink;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

use crate::configuration_registries::ASSETS;
use crate::host_app::{label_of, world as dimension_world};

const OVERWORLD: &str = "minecraft:overworld";
const TICKS: usize = 60;
const PATIENCE: Duration = Duration::from_secs(120);

fn server(worlds: Placement) -> App {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.worlds = worlds;
    plugins.network.announce_on_lan = false;
    plugins.worlds.seed = 4;
    let mut app = App::new();
    app.add_plugins(plugins.build());
    app.finish();
    app.cleanup();
    crate::support::update_until_playing(&mut app);
    runner::between_ticks(&mut app);
    app
}

fn tick(app: &mut App) {
    app.update();
    runner::between_ticks(app);
    runner::drain(app);
}

/// Waits between ticks until the generation and light steps that the tick started have ended.
fn wait_for_steps(app: &mut App, label: Entity) {
    let world = dimension_world(app, label);
    if let Some(mut generation) = world.get_resource_mut::<GenerationLink>() {
        block_on(generation.answered());
    }
    if let Some(mut light) = world.get_resource_mut::<LightLink>() {
        block_on(light.answered());
    }
}

fn dimension_entity(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<ColumnIndex>>()
        .single(world)
        .expect("the dimension entity")
}

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

fn hash_light(
    hasher: &mut DefaultHasher,
    field: &mut [u8; SectionPos::VOLUME],
    light: Option<&LightStorage>,
) {
    match light {
        Some(storage) => {
            storage.write_field(field);
            field.hash(hasher);
        }
        None => 0u8.hash(hasher),
    }
}

/// The simulation-visible state of the dimension: every section with its stage, light and, as
/// `hashed` asks, its blocks; every column with its heightmaps; the block entities and the
/// players. Entities enter by id, so a different allocation order is a different world.
fn world_hash(world: &mut World, hashed: Hashed) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut field = [0u8; SectionPos::VOLUME];
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
        hash_light(&mut hasher, &mut field, block.map(|light| &light.0));
        hash_light(&mut hasher, &mut field, sky.map(|light| &light.0));
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

/// The hash of the dimension after each tick of the scenario, and of its blocks after the last.
fn run(worlds: Placement) -> Vec<u64> {
    let mut app = server(worlds);
    let label = label_of(&app, OVERWORLD);
    // The generation context comes from assets, which load off the tick; nothing generates
    // before the player arrives, so the ticks it takes are not part of the scenario.
    let deadline = Instant::now() + PATIENCE;
    while !dimension_world(&mut app, label).contains_resource::<FillContext>() {
        assert!(
            Instant::now() < deadline,
            "the generation context never loaded"
        );
        tick(&mut app);
    }
    let blocks = app.world().resource::<Blocks>().clone();
    let state = |name: &str| {
        blocks
            .block(name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .default_state_id
    };
    let world = dimension_world(&mut app, label);
    let dimension = dimension_entity(world);
    let player = world
        .spawn((
            Player,
            Transform::from_translation(DVec3::new(8.5, 100.0, 8.5)),
            PlayerViewDistance {
                distance: 2,
                ..PlayerViewDistance::default()
            },
            ColumnView::default(),
            InDimension(dimension),
            Owner(PlayerSession(1)),
        ))
        .id();
    let scenario = Scenario {
        dimension,
        player,
        glowstone: state("minecraft:glowstone"),
        stone: state("minecraft:stone"),
    };

    app.init_resource::<TileDeltaLog>();
    let mut published_tiles = 0;
    let mut hashes = Vec::with_capacity(TICKS + 1);
    for at in 0..TICKS {
        scenario.script(dimension_world(&mut app, label), at);
        tick(&mut app);
        wait_for_steps(&mut app, label);
        let mut hasher = DefaultHasher::new();
        world_hash(dimension_world(&mut app, label), Hashed::WithoutBlocks).hash(&mut hasher);
        // The tile deltas are a pure output of the tick, so the runs publish the same ones.
        let logged = std::mem::take(&mut app.world_mut().resource_mut::<TileDeltaLog>().0);
        for entry in logged.iter().filter(|entry| entry.dimension == label) {
            hash_debug(&mut hasher, &entry.deltas);
            published_tiles += entry.deltas.tiles.len();
        }
        hashes.push(hasher.finish());
    }
    hashes.push(world_hash(
        dimension_world(&mut app, label),
        Hashed::WithBlocks,
    ));
    let world = dimension_world(&mut app, label);
    let loaded = world
        .query::<&SectionStage>()
        .iter(world)
        .filter(|stage| **stage == SectionStage::Loaded)
        .count();
    assert!(
        loaded > 0,
        "the scenario generated nothing, so it compares nothing"
    );
    assert!(
        published_tiles > 0,
        "the player published no tile delta, so the deltas compare nothing"
    );
    hashes
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
    for worlds in [Placement::Thread, Placement::Thread] {
        let other = run(worlds);
        assert_eq!(
            first_divergence(&inline, &other),
            None,
            "{worlds:?} diverges from Inline at the tick shown (the last entry holds the blocks)"
        );
    }
}
