//! The world a tick produces must not depend on the thread count or on the order in which the
//! generation and light steps ended.
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
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_engine::column_view::ColumnView;
use mcrs_minecraft_engine_server::{TickThreading, runner};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick_threading;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::outbox_drain::OutboxDrain;
use mcrs_minecraft_engine_world::world::storage::block_entity::BlockEntity;
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_minecraft_engine_world::world::sub_app::DimAppLabel;
use mcrs_minecraft_heightmap::{MotionHeightmap, SurfaceHeightmap};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_light::LightLink;
use mcrs_minecraft_vanilla_worldgen::generation_is_idle;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

use crate::configuration_registries::ASSETS;
use crate::host_app::label_of;

const OVERWORLD: &str = "minecraft:overworld";
const TICKS: usize = 60;
const PATIENCE: Duration = Duration::from_secs(120);

fn server(threading: TickThreading) -> App {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.tick_threading = threading;
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
    tick_threading::update(app);
    runner::between_ticks(app);
    runner::drain_woken(app);
}

fn dimension_world(app: &mut App, label: Entity) -> &mut World {
    app.sub_app_mut(DimAppLabel(label)).world_mut()
}

fn steps_ended(world: &mut World) -> bool {
    let light_idle = world
        .get_resource_mut::<LightLink>()
        .is_none_or(|mut light| {
            light.take_in();
            light.is_idle()
        });
    generation_is_idle(world) && light_idle
}

/// Waits as the server loop does between ticks, draining the dimension's outbox meanwhile: the
/// drain may run at any moment between ticks, so it must not change what the next tick sees.
fn wait_for_steps(app: &mut App, label: Entity) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        dimension_world(app, label).run_schedule(OutboxDrain);
        runner::drain_woken(app);
        if steps_ended(dimension_world(app, label)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the generation and light steps never caught up with the tick"
        );
        std::thread::sleep(Duration::from_millis(1));
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
fn run(threading: TickThreading) -> Vec<u64> {
    let mut app = server(threading);
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

    let mut hashes = Vec::with_capacity(TICKS + 1);
    for at in 0..TICKS {
        scenario.script(dimension_world(&mut app, label), at);
        tick(&mut app);
        wait_for_steps(&mut app, label);
        hashes.push(world_hash(
            dimension_world(&mut app, label),
            Hashed::WithoutBlocks,
        ));
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
    let single = run(TickThreading::Single);
    for threading in [TickThreading::Parallel, TickThreading::Parallel] {
        let other = run(threading);
        assert_eq!(
            first_divergence(&single, &other),
            None,
            "{threading:?} diverges from Single at the tick shown (the last entry holds the blocks)"
        );
    }
}
