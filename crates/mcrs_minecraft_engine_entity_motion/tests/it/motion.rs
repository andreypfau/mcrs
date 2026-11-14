use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::world::World;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::EntityMotionPlugin;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

/// A dimension from `y = -64` with the floor block under `y = 0` and air above it, its sections
/// loaded from `x, z = -16` to `31`, and the motion plugin alone.
fn app(floor: fn(&TestBlocks) -> BlockStateId) -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let blocks = TestBlocks::new();
    let floor = floor(&blocks);
    let mut app = App::new();
    app.configure_sets(
        FixedUpdate,
        (
            TickSet::DomainInput,
            TickSet::ClientInput,
            TickSet::Simulate,
            TickSet::WorldWrite,
            TickSet::Project,
            TickSet::Output,
        )
            .chain(),
    )
    .insert_resource(PhysicsTables::new(Tables::new(blocks.table)))
    .add_plugins(EntityMotionPlugin);
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::default()
        });
    });
    let world = app.world_mut();
    let mut index = SectionIndex::new();
    for x in -1..=1 {
        for z in -1..=1 {
            for y in -4..=3 {
                let state = if y < 0 { floor } else { BlockStateId::AIR };
                let pos = SectionPos::new(x, y, z);
                let section = world.spawn_empty().id();
                index.insert(pos, section);
                index.land(
                    pos,
                    section,
                    ChunkBlocks::new(SectionStates::homogeneous(state)),
                    None,
                    false,
                );
            }
        }
    }
    world.spawn((Dimension, DimensionTypeConfig::new(-64, 384), index));
    app
}

fn spawn(world: &mut World, row: MotionRow, position: DVec3) -> Entity {
    let body = Body {
        position,
        ..Body::default()
    };
    world
        .spawn((
            MotionState(EntityState::new(EntityKey(7), body, None)),
            MotionKind(row),
        ))
        .id()
}

fn tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

fn state(app: &App, entity: Entity) -> EntityState {
    app.world()
        .get::<MotionState>(entity)
        .expect("a moving entity")
        .0
}

#[test]
fn a_block_placed_under_a_resting_item_wakes_it() {
    let mut app = app(|blocks| blocks.stone);
    let item = spawn(
        app.world_mut(),
        MotionRow::item(),
        DVec3::new(0.5, 0.0, 0.5),
    );
    for _ in 0..100 {
        tick(&mut app);
    }
    let rest = state(&app, item).rest;
    assert!(matches!(rest, Rest::Resting(_)), "the item rests: {rest:?}");

    let stone = TestBlocks::new().stone;
    app.world_mut().write_message(BlockPlaced {
        chunk: Entity::PLACEHOLDER,
        chunk_pos: SectionPos::new(0, -1, 0),
        block_pos: BlockPos::new(0, -1, 0),
        old_state: stone,
        new_state: stone,
        flags: BlockUpdateFlags::empty(),
    });
    tick(&mut app);

    let rest = state(&app, item).rest;
    assert!(
        !matches!(rest, Rest::Resting(_)),
        "the item wakes: {rest:?}"
    );
}
