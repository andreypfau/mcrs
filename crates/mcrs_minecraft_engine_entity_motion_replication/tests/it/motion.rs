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
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_entity_motion_replication::MotionId;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::ReplicationData;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

fn app(floor: fn(&TestBlocks) -> BlockStateId) -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let blocks = TestBlocks::new();
    let floor = floor(&blocks);
    let mut ids = CellEntityIds::default();
    ids.add(EntityIdRange::starting_at(2).expect("a small range"));
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
    .insert_resource(ids)
    .insert_resource(PhysicsTables::new(Tables::new(blocks.table)))
    .add_plugins((EntityMotionPlugin, EntityMotionReplicationPlugin));
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
            MotionState(EntityState::new(EntityUuid::from_u128(7), body, None)),
            MotionKind(row),
            Appearance {
                kind: &EntityTypeData::ITEM,
                look: SpawnLook::Mob(MobLooks::default()),
            },
        ))
        .id()
}

fn tick(app: &mut App) -> Vec<&'static str> {
    let world = app.world_mut();
    world.run_schedule(FixedUpdate);
    let deltas = world.resource_mut::<TileEvents>().take();
    let mut kinds = Vec::new();
    for delta in deltas.tiles {
        for event in delta.events {
            kinds.push(match event.event {
                EntityEvent::Spawn(_) => "spawn",
                EntityEvent::Move(_) => "move",
                EntityEvent::Data(_) => "data",
                EntityEvent::Enter { .. } => "enter",
                EntityEvent::Leave { .. } => "leave",
                EntityEvent::Despawn => "despawn",
                _ => "other",
            });
        }
    }
    kinds
}

fn state(app: &App, entity: Entity) -> EntityState {
    app.world()
        .get::<MotionState>(entity)
        .expect("a moving entity")
        .0
}

#[test]
fn a_falling_block_lands_on_the_floor_and_falls_silent_at_rest() {
    let mut app = app(|blocks| blocks.stone);
    let block = spawn(
        app.world_mut(),
        MotionRow::falling_block(),
        DVec3::new(0.5, 3.0, 0.5),
    );
    let mut ticks = Vec::new();
    for _ in 0..60 {
        let events = tick(&mut app);
        let step = *app.world().get::<StepOutcome>(block).expect("a step");
        ticks.push((events, step, state(&app, block)));
    }

    assert_eq!(
        ticks[0].0,
        ["spawn", "move"],
        "the first tick spawns the entity, then moves it"
    );
    let landing = ticks
        .iter()
        .position(|(_, step, _)| step.kernel.landed)
        .expect("the block lands");
    assert!(
        ticks[landing].2.body.flags.on_ground,
        "it lands on the ground"
    );
    let last_event = ticks
        .iter()
        .rposition(|(events, _, _)| !events.is_empty())
        .expect("events");
    assert!(last_event >= landing, "the landing is published");
    for (at, (events, _, _)) in ticks.iter().enumerate().take(last_event + 1).skip(1) {
        assert_eq!(events, &["move"], "tick {at} moves the block");
    }
    assert!(
        last_event + 20 < ticks.len(),
        "the block rests and publishes nothing after tick {last_event}"
    );
    let rested = state(&app, block);
    assert!(matches!(rested.rest, Rest::Resting(_)), "{:?}", rested.rest);
    assert_eq!(rested.body.position.y, 0.0, "it rests on the floor");
}

#[test]
fn an_item_at_rest_on_the_floor_publishes_nothing() {
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

    for at in 0..40 {
        assert_eq!(
            tick(&mut app),
            [""; 0],
            "tick {at} at rest publishes nothing"
        );
    }
}

#[test]
fn an_item_that_bounces_on_slime_reports_the_bounce_in_that_tick_only() {
    let mut app = app(|blocks| blocks.slime);
    let item = spawn(
        app.world_mut(),
        MotionRow::item(),
        DVec3::new(0.5, 3.0, 0.5),
    );
    tick(&mut app);
    let mut bounces = 0;
    for at in 1..60 {
        let before = state(&app, item).body.velocity.y;
        app.world_mut().run_schedule(FixedUpdate);
        let deltas = app.world_mut().resource_mut::<TileEvents>().take();
        let after = state(&app, item).body.velocity.y;
        let reported = deltas
            .tiles
            .iter()
            .flat_map(|delta| &delta.events)
            .find_map(|event| match &event.event {
                EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => {
                    Some(state.bounced)
                }
                _ => None,
            });
        let turned_up = before <= 0.0 && after > 0.0;
        if turned_up {
            assert_eq!(
                reported,
                Some(true),
                "tick {at} reports its bounce: velocity {before} to {after}"
            );
            bounces += 1;
        } else if let Some(bounced) = reported {
            assert!(
                !bounced,
                "tick {at} does not bounce: velocity {before} to {after}"
            );
        }
    }
    assert!(bounces > 0, "the item bounces on the slime block");
}

#[test]
fn an_entity_without_an_appearance_waits_unseen_and_spawns_with_it() {
    let mut app = app(|blocks| blocks.stone);
    let start = DVec3::new(0.5, 3.0, 0.5);
    let block = spawn(app.world_mut(), MotionRow::falling_block(), start);
    let appearance = app
        .world_mut()
        .entity_mut(block)
        .take::<Appearance>()
        .expect("an appearance");

    assert!(tick(&mut app).is_empty(), "no event without an appearance");
    assert!(app.world().get::<MotionId>(block).is_none(), "no id");
    assert_eq!(state(&app, block).body.position, start, "it does not move");

    app.world_mut().entity_mut(block).insert(appearance);
    assert_eq!(tick(&mut app), ["spawn", "move"]);
}

#[test]
fn falling_block_spawn_shows_start_then_first_step_as_move() {
    let mut app = app(|blocks| blocks.stone);
    let start = DVec3::new(0.5, 3.0, 0.5);
    spawn(app.world_mut(), MotionRow::falling_block(), start);

    app.world_mut().run_schedule(FixedUpdate);

    let deltas = app.world_mut().resource_mut::<TileEvents>().take();
    let states: Vec<(&str, DVec3, DVec3)> = deltas
        .tiles
        .iter()
        .flat_map(|delta| &delta.events)
        .filter_map(|event| match &event.event {
            EntityEvent::Spawn(ReplicationData::Mob(MobReport::Spawn(spawn))) => {
                Some(("spawn", spawn.state.position, spawn.state.velocity))
            }
            EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => {
                Some(("move", state.position, state.velocity))
            }
            _ => None,
        })
        .collect();
    let [
        ("spawn", spawned, spawn_velocity),
        ("move", moved, move_velocity),
    ] = states[..]
    else {
        panic!("a spawn, then a move: {states:?}");
    };
    assert_eq!(
        (spawned, spawn_velocity),
        (start, DVec3::ZERO),
        "FBK-SPN-001: the spawn shows the state before the first step"
    );
    assert!(
        moved.y < start.y && move_velocity.y < 0.0,
        "the move shows the first step: {moved:?}, {move_velocity:?}"
    );
}

fn moves(app: &mut App) -> Vec<DVec3> {
    app.world_mut().run_schedule(FixedUpdate);
    let deltas = app.world_mut().resource_mut::<TileEvents>().take();
    deltas
        .tiles
        .iter()
        .flat_map(|delta| &delta.events)
        .filter_map(|event| match &event.event {
            EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => Some(state.position),
            _ => None,
        })
        .collect()
}

#[test]
fn a_write_of_another_system_to_an_entity_at_rest_is_published_once() {
    let mut app = app(|blocks| blocks.stone);
    let block = spawn(
        app.world_mut(),
        MotionRow::falling_block(),
        DVec3::new(0.5, 3.0, 0.5),
    );
    app.world_mut()
        .get_mut::<MotionState>(block)
        .expect("a moving entity")
        .0
        .no_gravity = true;
    for _ in 0..5 {
        tick(&mut app);
    }
    assert!(moves(&mut app).is_empty(), "the block floats still");

    let teleport = DVec3::new(1.5, 3.0, 0.5);
    app.world_mut()
        .get_mut::<MotionState>(block)
        .expect("a moving entity")
        .0
        .body
        .position = teleport;

    assert_eq!(moves(&mut app), [teleport], "the write reaches the viewers");
    assert!(moves(&mut app).is_empty(), "the write is published once");
}

#[test]
fn a_write_to_a_held_entity_at_rest_wakes_it_after_the_release() {
    let mut app = app(|blocks| blocks.stone);
    let block = spawn(
        app.world_mut(),
        MotionRow::falling_block(),
        DVec3::new(0.5, 3.0, 0.5),
    );
    for _ in 0..60 {
        tick(&mut app);
    }
    let rest = state(&app, block).rest;
    assert!(
        matches!(rest, Rest::Resting(_)),
        "the block rests: {rest:?}"
    );

    let mut entity = app.world_mut().entity_mut(block);
    entity.insert(MotionHold);
    entity
        .get_mut::<MotionState>()
        .expect("a moving entity")
        .0
        .body
        .velocity = DVec3::new(0.0, 0.5, 0.0);
    tick(&mut app);
    app.world_mut().entity_mut(block).remove::<MotionHold>();
    tick(&mut app);

    assert!(
        state(&app, block).body.position.y > 0.0,
        "the write survives the hold and lifts the block"
    );
}
