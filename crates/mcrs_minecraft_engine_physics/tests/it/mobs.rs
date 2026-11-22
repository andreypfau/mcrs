#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::Movers;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::MobExtras;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_physics::MobSpawnRequest;
use mcrs_minecraft_engine_physics::SpawnSetup;
use mcrs_minecraft_engine_physics::Spawned;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::mob_body;
use mcrs_minecraft_engine_physics::mob_row;
use mcrs_minecraft_engine_physics::mob_vitals;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;

fn pig(scene: &mut TestScene, id: u64, position: DVec3) -> Entity {
    let row = mob_row(&EntityTypeData::PIG);
    let key = EntityUuid::from_u128(u128::from(id));
    let state = mob_body(&EntityTypeData::PIG, &row, key, position);
    let vitals = mob_vitals(&EntityTypeData::PIG);
    scene.spawn_with_vitals(EntityId(id), state, row, vitals)
}

fn position(scene: &TestScene, entity: Entity) -> DVec3 {
    scene.state(entity).expect("the pig stays").body.position
}

fn steer(scene: &mut TestScene, entity: Entity, intent: MobIntent) {
    let world = scene.app.world_mut();
    *world
        .get_mut::<MobIntent>(entity)
        .expect("a mob takes intents") = intent;
}

fn walker(scene: &mut TestScene, key: u128) -> Entity {
    let states = scene.states().into_iter();
    states
        .filter(|(_, state)| state.key == EntityUuid::from_u128(key))
        .map(|(entity, _)| entity)
        .next()
        .expect("the walker is in the scene")
}

#[test]
fn psh_two_pigs_in_one_point_push_each_other_apart_on_the_floor() {
    let mut scene = TestScene::new(1, 0, 0, 1);
    let start = DVec3::new(5.5, f64::from(FLOOR) + 2.0, 5.5);
    let first = pig(&mut scene, 2, start);
    let second = pig(&mut scene, 4, start + DVec3::new(0.01, 0.0, 0.0));

    for _ in 0..60 {
        scene.step();
    }

    let [first, second] = [first, second].map(|entity| position(&scene, entity));
    assert!(
        (first.x - second.x).abs() > 0.5,
        "the push separates the boxes: {first} and {second}"
    );
    assert_eq!(
        [first.y, second.y],
        [f64::from(FLOOR); 2],
        "both fall onto the floor"
    );
}

#[test]
fn an_intent_steers_a_pig_before_its_step() {
    let mut scene = TestScene::new(1, 0, 0, 1);
    let start = DVec3::new(5.5, f64::from(FLOOR), 5.5);
    let pig = pig(&mut scene, 2, start);
    let forward = Steering {
        input: MoveInput {
            forward: 1.0,
            ..MoveInput::default()
        },
        jump: false,
        speed: 0.25,
    };

    for _ in 0..20 {
        let intent = MobIntent {
            steering: Some(forward),
            yaw: Some(0.0),
            ..MobIntent::default()
        };
        steer(&mut scene, pig, intent);
        scene.step();
    }

    let end = position(&scene, pig);
    assert!(
        end.z - start.z > 1.0 && (end.x - start.x).abs() < 1e-9,
        "the yaw 0 walks to +z: {start} to {end}"
    );
}

#[test]
fn an_intent_that_steers_a_resting_walker_wakes_it() {
    let mut scene = TestScene::new(1, 2, 0, 7);
    let standing = walker(&mut scene, 1);
    for _ in 0..60 {
        scene.step();
    }
    let start = position(&scene, standing);
    let intent = MobIntent {
        steering: Some(Steering {
            input: MoveInput {
                forward: 1.0,
                ..MoveInput::default()
            },
            jump: false,
            speed: 0.1,
        }),
        ..MobIntent::default()
    };
    steer(&mut scene, standing, intent);
    for _ in 0..5 {
        scene.step();
    }
    let end = position(&scene, standing);
    assert!(
        start.distance(end) > 0.1,
        "the walker walks from {start} to {end}"
    );
}

#[test]
fn an_intent_that_only_turns_the_head_publishes_a_move() {
    let mut scene = TestScene::new(1, 2, 0, 7);
    let standing = walker(&mut scene, 1);
    let moved = |scene: &TestScene| {
        let movers = scene.app.world().resource::<Movers>();
        movers.entities().contains(&standing)
    };
    for _ in 0..60 {
        scene.step();
    }
    assert!(!moved(&scene), "the standing walker settled on the floor");
    let intent = MobIntent {
        head_yaw: Some(45.0),
        ..MobIntent::default()
    };
    steer(&mut scene, standing, intent);
    scene.step();
    assert!(moved(&scene), "the turn of the head reaches the viewers");
}

#[derive(Resource, Default)]
struct Published {
    spawned: Vec<Spawned>,
}

fn publish(changes: Res<MobChanges>, mut published: ResMut<Published>) {
    published.spawned.extend_from_slice(changes.spawned());
}

#[test]
fn a_spawn_request_makes_one_mob_per_uuid_and_a_death_despawns_it() {
    let mut scene = TestScene::new(1, 0, 0, 1);
    scene.app.init_resource::<Published>();
    scene
        .app
        .add_systems(FixedUpdate, publish.in_set(MobPublish));
    let ids = EntityIdRange::starting_at(100).expect("the range fits");
    scene
        .app
        .world_mut()
        .resource_mut::<CellEntityIds>()
        .add(ids);
    let request = |uuid: u128, x: f64| MobSpawnRequest {
        kind: &EntityTypeData::PIG,
        position: DVec3::new(x, f64::from(FLOOR), 5.5),
        uuid,
        setup: SpawnSetup::None,
        extras: MobExtras::default(),
    };
    for (uuid, x) in [(9, 5.5), (3, 9.5), (9, 12.5)] {
        scene.app.world_mut().write_message(request(uuid, x));
    }
    scene.step();

    let index = scene.app.world().resource::<MobIndex>();
    let [first, second] = [3, 9].map(|uuid| {
        index
            .get(EntityUuid::from_u128(uuid))
            .expect("the mob is indexed by its UUID")
    });
    let world = scene.app.world();
    let ids = [first, second].map(|entity| world.get::<MotionId>(entity).map(|id| id.0));
    assert_eq!(
        ids,
        [Some(EntityId(100)), Some(EntityId(102))],
        "ids in UUID order"
    );
    let uuids = [first, second].map(|entity| world.get::<Mob>(entity).map(|mob| mob.uuid));
    assert_eq!(uuids, [Some(3), Some(9)]);
    let spawned: Vec<Entity> = (world.resource::<Published>().spawned.iter())
        .map(|spawned| spawned.entity)
        .collect();
    assert_eq!(spawned, [first, second]);
    assert_eq!(
        scene.states().len(),
        2,
        "the second request for UUID 9 is refused"
    );

    let tick = scene.tick;
    scene
        .app
        .world_mut()
        .resource_mut::<PendingHits>()
        .push(DamageRequest::new(
            first,
            Hit::new(100.0, DamageSource::of(0)),
            WriterKey::Command { sequence: 0 },
            tick,
            0,
        ));
    scene.step();
    let outcome = (scene.app.world().resource::<Messages<Damaged>>())
        .iter_current_update_messages()
        .find(|damaged| damaged.target == first)
        .map(|damaged| damaged.report.outcome);
    assert!(
        outcome.is_some_and(|outcome| outcome != HitOutcome::Rejected),
        "the mob is living"
    );
    for _ in 0..25 {
        scene.step();
    }
    let world = scene.app.world();
    assert!(
        world.get_entity(first).is_err(),
        "the dead mob is despawned"
    );
    assert_eq!(
        world.resource::<MobIndex>().get(EntityUuid::from_u128(3)),
        None,
        "its UUID is free"
    );

    scene.app.world_mut().despawn(second);
    assert_eq!(
        scene
            .app
            .world()
            .resource::<MobIndex>()
            .get(EntityUuid::from_u128(9)),
        None,
        "a mob despawned from outside the pass frees its UUID too"
    );
}
