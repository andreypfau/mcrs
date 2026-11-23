use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::ResMut;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_damage::DamagePlugin;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::CrowdCandidates;
use mcrs_minecraft_engine_entity_motion::Crowding;
use mcrs_minecraft_engine_entity_motion::Expired;
use mcrs_minecraft_engine_entity_motion::Expiry;
use mcrs_minecraft_engine_entity_motion::ExpiryCause;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionPassPlugin;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::MotionStep;
use mcrs_minecraft_engine_entity_motion::MotionSteps;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TileBodies;
use mcrs_minecraft_physics::grid_row;

#[derive(Resource)]
struct Ground {
    fixture: TestFixture,
    crowding: Crowding,
    tick: u64,
}

fn drive(mut ground: ResMut<Ground>, mut step: MotionStep) {
    ground.tick += 1;
    let Ground {
        fixture,
        crowding,
        tick,
    } = &*ground;
    step.run(
        &fixture.world,
        &fixture.tables,
        Side::Server { authority: true },
        *tick,
        crowding,
    );
}

fn grid_app() -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let mut app = App::new();
    app.add_plugins(MotionPassPlugin);
    app
}

fn app(crowding: Crowding) -> App {
    let mut app = grid_app();
    app.insert_resource(Ground {
        fixture: TestFixture::new(),
        crowding,
        tick: 0,
    });
    app.add_systems(FixedUpdate, drive.in_set(MotionSteps));
    app
}

fn mob(app: &mut App, index: u32, position: DVec3) -> Entity {
    let motion = MotionRow::zombie();
    let mut state = EntityState::new(
        EntityUuid::from_u128(u128::from(index)),
        Body {
            position,
            ..Body::default()
        },
        Some(Living::of(&motion)),
    );
    state.no_gravity = true;
    let id = MotionId(EntityId(u64::from(index)));
    let vitals = Vitals::default();
    let entity = (app.world_mut()).spawn((MotionState(state), MotionKind(motion), id, vitals));
    entity.id()
}

fn tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

fn state(app: &App, entity: Entity) -> EntityState {
    app.world().get::<MotionState>(entity).expect("a mob").0
}

fn edit(app: &mut App, entity: Entity, change: impl FnOnce(&mut EntityState)) {
    let mut state = (app.world_mut().get_mut::<MotionState>(entity)).expect("a mob");
    change(&mut state.0);
}

fn die(app: &mut App, entity: Entity) {
    let mut vitals = (app.world_mut().get_mut::<Vitals>(entity)).expect("a mob");
    vitals.death_time = Some(0);
}

#[test]
fn two_overlapping_mobs_push_each_other_apart() {
    let mut app = app(Crowding::default());
    let left = mob(&mut app, 1, DVec3::new(0.5, 64.0, 0.5));
    let right = mob(&mut app, 2, DVec3::new(0.7, 64.0, 0.5));
    let far = mob(&mut app, 3, DVec3::new(20.5, 64.0, 0.5));
    tick(&mut app);
    let [left, right, far] = [left, right, far].map(|entity| state(&app, entity).body);
    assert!(left.velocity.x < 0.0, "{left:?}");
    assert_eq!(right.velocity.x, -left.velocity.x);
    assert_eq!(far.velocity, DVec3::ZERO);
}

#[test]
fn the_body_grid_equals_a_full_rebuild_after_spawn_move_death_and_despawn() {
    let mut app = grid_app();
    let mobs: Vec<Entity> = [0.5, 3.0, 31.5, 40.0, -10.0]
        .into_iter()
        .zip(1..)
        .map(|(x, index)| mob(&mut app, index, DVec3::new(x, 64.0, 0.5)))
        .collect();
    tick(&mut app);
    assert_grid_rebuilt(&mut app);
    let [first, second, third, fourth, fifth] = mobs[..] else {
        panic!("five mobs");
    };
    edit(&mut app, first, |state| state.body.position.x = 70.0);
    die(&mut app, second);
    app.world_mut().despawn(third);
    app.world_mut().entity_mut(fourth).remove::<MotionId>();
    app.world_mut().entity_mut(fifth).remove::<MotionKind>();
    mob(&mut app, 6, DVec3::new(-10.2, 64.0, 0.5));
    tick(&mut app);
    assert_grid_rebuilt(&mut app);
}

#[test]
fn the_body_grid_equals_a_full_rebuild_while_mobs_push_each_other() {
    let mut app = app(Crowding::default());
    for (index, x) in (1..).zip([0.5, 0.7, 0.9]) {
        mob(&mut app, index, DVec3::new(x, 64.0, 0.5));
    }
    tick(&mut app);
    for _ in 0..5 {
        let before = rebuilt_tiles(&mut app);
        tick(&mut app);
        assert_eq!(grid_tiles(&app), before);
    }
}

const TILES: [TileKey; 6] = [
    TileKey::new(-2, 0, 0),
    TileKey::new(-1, 0, 0),
    TileKey::new(0, 0, 0),
    TileKey::new(1, 0, 0),
    TileKey::new(2, 0, 0),
    TileKey::new(3, 0, 0),
];

fn assert_grid_rebuilt(app: &mut App) {
    let expected = rebuilt_tiles(app);
    assert_eq!(grid_tiles(app), expected);
}

fn rebuilt_tiles(app: &mut App) -> Vec<Vec<GridRow>> {
    let world = app.world_mut();
    let rows: Vec<GridRow> = (world
        .query::<(&MotionState, &MotionKind, &MotionId, Option<&Vitals>)>())
    .iter(world)
    .filter_map(|(state, kind, id, vitals)| {
        let flags = if vitals.is_none_or(Vitals::is_alive) {
            GridFlags::PUSHABLE
        } else {
            GridFlags::empty()
        };
        grid_row(&state.0, &kind.0, id.0, flags)
    })
    .collect();
    let rebuild = |tile: TileKey| {
        let inside: Vec<GridRow> = (rows.iter())
            .filter(|row| TileKey::at(row.position) == tile)
            .copied()
            .collect();
        let mut bodies = TileBodies::default();
        bodies.fill(tile, &inside);
        bodies.rows().to_vec()
    };
    TILES.map(rebuild).to_vec()
}

fn grid_tiles(app: &App) -> Vec<Vec<GridRow>> {
    let grid = app.world().resource::<BodyGrid>();
    TILES.map(|tile| grid.tile(tile).rows().to_vec()).to_vec()
}

#[test]
fn an_intent_turns_a_mob_once() {
    let mut app = app(Crowding::default());
    let zombie = mob(&mut app, 1, DVec3::new(0.5, 64.0, 0.5));
    app.world_mut().entity_mut(zombie).insert(MobIntent {
        yaw: Some(90.0),
        ..MobIntent::default()
    });
    tick(&mut app);
    let yaw = |app: &App| {
        state(app, zombie)
            .living
            .map(|living| living.orientation.yaw)
    };
    assert_eq!(yaw(&app), Some(90.0));
    edit(&mut app, zombie, |state| {
        if let Some(living) = state.living.as_mut() {
            living.orientation.yaw = 10.0;
        }
    });
    tick(&mut app);
    assert_eq!(yaw(&app), Some(10.0));
}

fn kill(app: &mut App, entity: Entity) {
    app.world_mut()
        .entity_mut(entity)
        .insert(HitRecord::default());
    app.world_mut()
        .resource_mut::<PendingHits>()
        .push(DamageRequest::new(
            entity,
            Hit::new(f32::MAX, DamageSource::of(0)),
            WriterKey::Command { sequence: 0 },
            0,
            0,
        ));
}

#[test]
fn a_dying_mob_expires_on_the_twentieth_tick() {
    let mut app = app(Crowding::default());
    app.add_plugins(DamagePlugin);
    let zombie = mob(&mut app, 1, DVec3::new(0.5, 64.0, 0.5));
    kill(&mut app, zombie);
    tick(&mut app);
    let vitals = app.world().get::<Vitals>(zombie).copied();
    assert_eq!(
        vitals.and_then(|vitals| vitals.death_time),
        Some(0),
        "the hit kills the mob after its step"
    );
    for _ in 1..20 {
        tick(&mut app);
        assert_eq!(app.world().resource::<Expired>().entries(), []);
    }
    tick(&mut app);
    let expected = Expiry {
        entity: zombie,
        key: EntityUuid::from_u128(1),
        cause: ExpiryCause::Death,
    };
    assert_eq!(app.world().resource::<Expired>().entries(), [expected]);
}

#[test]
fn crowd_candidates_come_out_in_uuid_order() {
    let crowding = Crowding {
        rules: PushRules {
            max_entity_cramming: 3,
            ..PushRules::default()
        },
        damage_type: Some(0),
    };
    let mut app = app(crowding);
    for index in (1..=8).rev() {
        mob(&mut app, index, DVec3::new(0.5, 64.0, 0.5));
    }
    let mut most = 0;
    for _ in 0..20 {
        tick(&mut app);
        let keys: Vec<EntityUuid> = (app.world().resource::<CrowdCandidates>().entries())
            .iter()
            .map(|candidate| candidate.own.key)
            .collect();
        assert!(keys.is_sorted(), "{keys:?}");
        most = most.max(keys.len());
    }
    assert!(most >= 2, "a crowded tick has two candidates or more");
}
