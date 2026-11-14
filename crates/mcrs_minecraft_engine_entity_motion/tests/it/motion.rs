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
use mcrs_minecraft_engine_entity_motion::Appearance;
use mcrs_minecraft_engine_entity_motion::EntityMotionPlugin;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityKey;
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
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

/// A dimension from `y = -64` with the floor block under `y = 0` and air above it, its sections
/// loaded from `x, z = -16` to `31`, and the motion plugin.
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
            Appearance {
                kind: &EntityTypeData::ITEM,
                look: SpawnLook::Mob(MobLooks::default()),
            },
        ))
        .id()
}

/// Runs one tick and gives the kinds of its tile events.
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
        ticks.push((events, step));
    }

    assert_eq!(ticks[0].0, ["spawn"], "the first tick spawns the entity");
    let landing = ticks
        .iter()
        .position(|(_, step)| step.kernel.landed)
        .expect("the block lands");
    assert!(ticks[landing].1.flags.on_ground, "it lands on the ground");
    let last_event = ticks
        .iter()
        .rposition(|(events, _)| !events.is_empty())
        .expect("events");
    assert!(last_event >= landing, "the landing is published");
    for (at, (events, _)) in ticks.iter().enumerate().take(last_event + 1).skip(1) {
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
        let Some(bounced) = reported else {
            continue;
        };
        let turned_up = before <= 0.0 && after > 0.0;
        assert_eq!(
            bounced, turned_up,
            "tick {at}: velocity {before} to {after}"
        );
        bounces += usize::from(bounced);
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
    assert_eq!(tick(&mut app), ["spawn"]);
}
