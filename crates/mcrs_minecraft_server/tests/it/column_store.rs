//! A column that goes cold keeps its last version in the data layer, and a column that a ticket
//! loads again comes back from there instead of from the generator.

use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::column_view::ColumnView;
use mcrs_minecraft_engine_server::runner;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::configuration_registries::ASSETS;
use crate::host_app::{label_of, world as dimension_world};

const OVERWORLD: &str = "minecraft:overworld";
const PATIENCE: Duration = Duration::from_secs(120);

fn server() -> App {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
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

fn tick_until(app: &mut App, label: Entity, what: &str, done: impl Fn(&mut World) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done(dimension_world(app, label)) {
        assert!(Instant::now() < deadline, "{what} never happened");
        app.update();
        runner::between_ticks(app);
        runner::drain(app);
    }
}

fn dimension_entity(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<ColumnIndex>>()
        .single(world)
        .expect("the dimension entity")
}

fn block_at(world: &mut World, dimension: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let section = world
        .get::<SectionIndex>(dimension)?
        .get(SectionPos::from(pos))?;
    if world.get::<SectionStage>(section) != Some(&SectionStage::Loaded) {
        return None;
    }
    let blocks = world.get::<ChunkBlocks>(section)?;
    blocks.0.get_cell(
        pos.x.rem_euclid(16) as usize,
        pos.y.rem_euclid(16) as usize,
        pos.z.rem_euclid(16) as usize,
    )
}

fn holds_column(world: &mut World, dimension: Entity, column: ColumnPos) -> bool {
    world
        .get::<ColumnIndex>(dimension)
        .is_some_and(|index| index.contains_key(&column))
}

#[test]
fn a_change_survives_the_column_going_cold_and_coming_back() {
    let mut app = server();
    let label = label_of(&app, OVERWORLD);
    tick_until(&mut app, label, "the generation context loaded", |world| {
        world.contains_resource::<FillContext>()
    });
    let glowstone = app
        .world()
        .resource::<Blocks>()
        .block("minecraft:glowstone")
        .expect("glowstone is declared")
        .default_state_id;
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
    let pos = BlockPos::new(8, 250, 8);
    let column = ColumnPos::new(0, 0);

    // Nothing answers the batches, so the test takes each one as the client would.
    tick_until(&mut app, label, "the column was sent", |world| {
        let Some(mut view) = world.get_mut::<ColumnView>(player) else {
            return false;
        };
        view.acknowledge_batch(64.0);
        view.holds(column)
    });
    let world = dimension_world(&mut app, label);
    assert_ne!(block_at(world, dimension, pos), Some(glowstone));
    world
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos,
            new_state: glowstone,
            flags: BlockUpdateFlags::all(),
            recursion_left: 512,
        });
    tick_until(&mut app, label, "the block was placed", |world| {
        block_at(world, dimension, pos) == Some(glowstone)
    });

    let moved = |app: &mut App, x: f64| {
        let world = dimension_world(app, label);
        world
            .get_mut::<Transform>(player)
            .expect("the player stands somewhere")
            .translation
            .x = x;
    };
    moved(&mut app, 16.0 * 40.0);
    tick_until(&mut app, label, "the column unloaded", |world| {
        !holds_column(world, dimension, column)
    });
    moved(&mut app, 8.5);
    tick_until(&mut app, label, "the column loaded again", |world| {
        block_at(world, dimension, pos).is_some()
    });
    assert_eq!(
        block_at(dimension_world(&mut app, label), dimension, pos),
        Some(glowstone),
        "the column came back from the generator, not from the data layer"
    );
}
