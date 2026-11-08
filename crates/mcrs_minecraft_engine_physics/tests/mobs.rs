//! Pig rows in the entity pass: two pigs summoned into one point push each other apart, and an
//! intent steers a pig.

use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::MobIntent;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::mob_body;
use mcrs_minecraft_engine_physics::mob_row;
use mcrs_minecraft_engine_physics::tile_at;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::Birth;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Steering;

fn pig(scene: &mut TestScene, id: u64, position: DVec3) {
    let row = mob_row(MobType::Pig);
    let key = Birth::from(BirthKey::new(0, SourceKind::Command, 0, id as u32));
    scene.rows.spawn(
        EntityId(id),
        mob_body(MobType::Pig, &row, key, position),
        row,
    );
}

fn position(scene: &mut TestScene, id: u64) -> DVec3 {
    scene
        .rows
        .tiles_mut()
        .flat_map(|(_, rows)| rows.rows_mut().map(|row| (row.id, row.state.body.position)))
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
    let mut scene = TestScene::new(1, 0, 0, 1);
    let start = DVec3::new(5.5, f64::from(FLOOR) + 2.0, 5.5);
    pig(&mut scene, 2, start);
    pig(&mut scene, 4, start + DVec3::new(0.01, 0.0, 0.0));

    run(&mut scene, 60, |_| {});

    let [first, second] = [2, 4].map(|id| position(&mut scene, id));
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
            tile_at(at),
            MobIntent {
                steering: Some(forward),
                yaw: Some(0.0),
                ..MobIntent::new(EntityId(2))
            },
        );
    });

    let end = position(&mut scene, 2);
    assert!(
        end.z - start.z > 1.0 && (end.x - start.x).abs() < 1e-9,
        "the yaw 0 walks to +z: {start} to {end}"
    );
}
