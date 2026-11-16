//! Pig and walker rows in the entity pass: two pigs summoned into one point push each other
//! apart, and an intent steers a pig, wakes a resting walker and publishes a turn of the head.

#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::mob_body;
use mcrs_minecraft_engine_physics::mob_row;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::MobIntent;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;

fn pig(scene: &mut TestScene, id: u64, position: DVec3) {
    let row = mob_row(&EntityTypeData::PIG);
    let key = EntityKey(u128::from(id));
    scene.rows.spawn(
        EntityId(id),
        mob_body(&EntityTypeData::PIG, &row, key, position),
        row,
    );
}

fn position(scene: &TestScene, id: u64) -> DVec3 {
    scene
        .rows
        .tiles()
        .flat_map(|(_, rows)| rows.rows().map(|row| (row.id, row.state.body.position)))
        .find(|(row, _)| *row == EntityId(id))
        .map(|(_, position)| position)
        .expect("the pig keeps its row")
}

fn run(scene: &mut TestScene, ticks: u32, before: impl Fn(&mut TestScene)) {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    for _ in 0..ticks {
        before(scene);
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
}

#[test]
fn psh_two_pigs_in_one_point_push_each_other_apart_on_the_floor() {
    let mut scene = TestScene::<()>::new(1, 0, 0, 1);
    let start = DVec3::new(5.5, f64::from(FLOOR) + 2.0, 5.5);
    pig(&mut scene, 2, start);
    pig(&mut scene, 4, start + DVec3::new(0.01, 0.0, 0.0));

    run(&mut scene, 60, |_| {});

    let [first, second] = [2, 4].map(|id| position(&scene, id));
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
    let mut scene = TestScene::<()>::new(1, 0, 0, 1);
    let start = DVec3::new(5.5, f64::from(FLOOR), 5.5);
    pig(&mut scene, 2, start);
    let forward = Steering {
        input: MoveInput {
            forward: 1.0,
            ..MoveInput::default()
        },
        jump: false,
        speed: 0.25,
    };

    run(&mut scene, 20, |scene| {
        let at = position(scene, 2);
        scene.rows.steer(
            TileKey::at(at),
            MobIntent {
                steering: Some(forward),
                yaw: Some(0.0),
                ..MobIntent::new(EntityId(2))
            },
        );
    });

    let end = position(&scene, 2);
    assert!(
        end.z - start.z > 1.0 && (end.x - start.x).abs() < 1e-9,
        "the yaw 0 walks to +z: {start} to {end}"
    );
}

#[test]
fn an_intent_that_steers_a_resting_walker_wakes_it() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = TestScene::<()>::new(1, 2, 0, 7);
    let standing = EntityId(1);
    for _ in 0..60 {
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
    let (tile, start) = scene
        .rows
        .living_rows()
        .find(|(at, _)| at.id == standing)
        .map(|(at, state)| (at.tile, state.body.position))
        .expect("the standing walker has a row");
    scene.rows.steer(
        tile,
        MobIntent {
            steering: Some(Steering {
                input: MoveInput {
                    forward: 1.0,
                    ..MoveInput::default()
                },
                jump: false,
                speed: 0.1,
            }),
            ..MobIntent::new(standing)
        },
    );
    for _ in 0..5 {
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
    let end = scene
        .rows
        .living_rows()
        .find(|(at, _)| at.id == standing)
        .map(|(_, state)| state.body.position)
        .expect("the walker keeps its row");
    assert!(
        start.distance(end) > 0.1,
        "the walker walks from {start} to {end}"
    );
}

#[test]
fn an_intent_that_only_turns_the_head_publishes_a_move() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = TestScene::<()>::new(1, 2, 0, 7);
    let standing = EntityId(1);
    for _ in 0..60 {
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
    assert!(
        scene.rows.moved().iter().all(|moved| moved.id != standing),
        "the standing walker settled on the floor"
    );
    let tile = scene
        .rows
        .living_rows()
        .find(|(at, _)| at.id == standing)
        .map(|(at, _)| at.tile)
        .expect("the standing walker has a row");
    scene.rows.steer(
        tile,
        MobIntent {
            head_yaw: Some(45.0),
            ..MobIntent::new(standing)
        },
    );
    scene.step(&pool, 1, &mut EntityWrites::default());
    assert!(
        scene.rows.moved().iter().any(|moved| moved.id == standing),
        "the turn of the head reaches the viewers"
    );
}
