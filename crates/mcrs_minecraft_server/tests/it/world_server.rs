//! An embedded server with generated worlds, and the steps that tests take in its dimension
//! worlds between ticks.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_tasks::block_on;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::block_simulation::DimensionBlockRules;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine_server::{Placement, WorldCells, runner};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{PlayerViewDistance, SentChunkView};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_light::{ColdLightLink, LightLink};
use mcrs_minecraft_vanilla_worldgen::WorldGeneration;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::configuration_registries::ASSETS;

pub const OVERWORLD: &str = "minecraft:overworld";

const PATIENCE: Duration = Duration::from_secs(120);

/// How a test server runs its worlds. The muxer always runs inline, so a test reads the views
/// where the muxer serves them. The worlds generate the beta overworld: 8 sections high, about
/// three times cheaper than the vanilla preset, which also builds the nether and the end.
pub struct WorldServer {
    pub worlds: Placement,
    pub cells: u32,
    pub rules: DimensionBlockRules,
    pub save: Option<PathBuf>,
}

impl Default for WorldServer {
    fn default() -> Self {
        Self {
            worlds: Placement::Inline,
            cells: 1,
            rules: DimensionBlockRules::default(),
            save: None,
        }
    }
}

impl WorldServer {
    /// Builds the server, ticks it to `Playing` and spawns its dimensions.
    pub fn start(self) -> App {
        let mut plugins = MinecraftServerPlugins::embedded();
        plugins.core.asset_path = Some(ASSETS.to_owned());
        plugins.core.worlds = self.worlds;
        plugins.core.muxer = Placement::Inline;
        plugins.core.cells = WorldCells {
            cells: NonZeroU32::new(self.cells).expect("a world has a cell"),
        };
        plugins.network.announce_on_lan = false;
        plugins.worlds.seed = 4;
        plugins.worlds.preset = "beta".to_owned();
        plugins.worlds.world = self.save;
        let mut app = App::new();
        app.add_plugins(plugins.build());
        app.insert_resource(self.rules);
        app.finish();
        app.cleanup();
        crate::support::update_until_playing(&mut app);
        runner::between_ticks(&mut app);
        app
    }
}

/// The world of every cell of `label`, in the order of the cells.
pub fn cell_worlds(app: &mut App, label: Entity) -> Vec<&mut World> {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
        .map(|sub_app| sub_app.world_mut())
        .collect()
}

/// Ticks until the world of `label` can generate: its first cell holds the context that the
/// generation of the world runs with. Nothing generates before a player arrives, so the ticks
/// it takes are no part of a scenario.
pub fn wait_for_generation(app: &mut App, label: Entity) {
    let deadline = Instant::now() + PATIENCE;
    while !cell_worlds(app, label)
        .into_iter()
        .next()
        .is_some_and(|world| world.contains_resource::<FillContext>())
    {
        assert!(
            Instant::now() < deadline,
            "the generation context never loaded"
        );
        crate::host_app::tick(app);
    }
}

/// Waits between ticks until the generation, light and store steps of every cell have ended.
pub fn wait_for_steps(app: &mut App, label: Entity) {
    block_on(app.world_mut().resource_mut::<DimCells>().stores_answered());
    if let Some(mut generation) = app.world_mut().get_resource_mut::<WorldGeneration>() {
        block_on(generation.answered());
    }
    for world in cell_worlds(app, label) {
        if let Some(mut light) = world.get_resource_mut::<LightLink>() {
            block_on(light.answered());
        }
        if let Some(mut light) = world.get_resource_mut::<ColdLightLink>() {
            block_on(light.answered());
        }
    }
}

/// Runs one tick, waits for the steps it started and returns the packets it wrote for the
/// clients. The writes of the steps reach every run at the same tick, so two runs differ only
/// by what they tick.
pub fn lockstep_tick(app: &mut App, label: Entity) -> Vec<OutboundPlayerPacket> {
    let packets = crate::host_app::tick(app);
    wait_for_steps(app, label);
    packets
}

/// Ticks until `done` holds in the first cell of `label`.
pub fn tick_until(app: &mut App, label: Entity, what: &str, done: impl Fn(&mut World) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done(crate::host_app::world(app, label)) {
        assert!(Instant::now() < deadline, "{what} never happened");
        crate::host_app::tick(app);
    }
}

pub fn block_state(app: &App, name: &str) -> BlockStateId {
    app.world()
        .resource::<Blocks>()
        .block(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
        .default_state_id
}

pub fn dimension_of(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .expect("the dimension entity")
}

/// Spawns a player of `session` straight into `world`, with no client behind it, that sees
/// `view_distance` columns far.
pub fn spawn_viewer(
    world: &mut World,
    session: PlayerSession,
    position: DVec3,
    view_distance: u8,
) -> Entity {
    let dimension = dimension_of(world);
    world
        .spawn((
            Player,
            Transform::from_translation(position),
            PlayerViewDistance {
                distance: view_distance,
                ..PlayerViewDistance::default()
            },
            SentChunkView::default(),
            InDimension(dimension),
            Owner(session),
        ))
        .id()
}

/// The cell of `label` that holds the player of `session`, by index, and the player.
pub struct Holder {
    pub cell: usize,
    pub player: Entity,
}

pub fn holder(app: &mut App, label: Entity, session: PlayerSession) -> Option<Holder> {
    let mut found = None;
    for (cell, world) in cell_worlds(app, label).into_iter().enumerate() {
        let player = world
            .query_filtered::<(Entity, &Owner), With<Player>>()
            .iter(world)
            .find(|(_, owner)| owner.0 == session)
            .map(|(player, _)| player);
        if let Some(player) = player {
            assert!(found.is_none(), "two cells hold the player of {session:?}");
            found = Some(Holder { cell, player });
        }
    }
    found
}

/// Writes a block as the player of `actor` would: into the world that holds it, whoever owns
/// the column.
pub fn place(
    app: &mut App,
    label: Entity,
    actor: PlayerSession,
    pos: BlockPos,
    state: BlockStateId,
) {
    let Holder { cell, .. } = holder(app, label, actor).expect("the actor is in the world");
    let mut worlds = cell_worlds(app, label);
    let world = worlds.swap_remove(cell);
    let dimension = dimension_of(world);
    world
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos,
            new_state: state,
            flags: BlockUpdateFlags::ALL,
            recursion_left: 512,
        });
}

/// Moves the player of `session` by `by`.
pub fn walk(app: &mut App, label: Entity, session: PlayerSession, by: DVec3) {
    let Holder { cell, player } = holder(app, label, session).expect("the walker is in the world");
    let mut worlds = cell_worlds(app, label);
    worlds
        .swap_remove(cell)
        .get_mut::<Transform>(player)
        .expect("the walker stands somewhere")
        .translation += by;
}

/// Where the player of `session` stands.
pub fn position(app: &mut App, label: Entity, session: PlayerSession) -> DVec3 {
    let Holder { cell, player } = holder(app, label, session).expect("the player is in the world");
    let mut worlds = cell_worlds(app, label);
    worlds
        .swap_remove(cell)
        .get::<Transform>(player)
        .expect("the player stands somewhere")
        .translation
}
