//! A crowd that has room to spread settles: the push moves the bodies apart until no box
//! overlaps another, and then they rest (PHY-PSH-006, PHY-CMP-012, PHY-SKIP-006).

use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::Dimensions;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_voxel::BlockPos;

/// The half width of the inside of the pen, in blocks.
const PEN: i32 = 14;
/// Bodies per block of floor: boxes of 0.9 × 0.9 cover 65% of it, so they fit without overlap.
const DENSITY: f64 = 0.8;
const TICKS: u32 = 400;

/// Idle pigs at random places in a walled pen, with `max_entity_cramming` 0.
fn pen() -> TestScene {
    let mut scene = TestScene::new(1, 0, 0, 3);
    scene.push = PushRules {
        max_entity_cramming: 0,
        ..PushRules::default()
    };
    let stone = TestBlocks::new().stone;
    for along in -PEN - 1..=PEN {
        for y in FLOOR..FLOOR + 2 {
            for at in [
                BlockPos::new(along, y, -PEN - 1),
                BlockPos::new(along, y, PEN),
                BlockPos::new(-PEN - 1, y, along),
                BlockPos::new(PEN, y, along),
            ] {
                scene
                    .blocks
                    .set_block(at, stone)
                    .expect("the wall is inside the floor");
            }
        }
    }
    let width = f64::from(2 * PEN);
    let count = (width * width * DENSITY) as u32;
    let motion = MotionRow::living(Category::Walker, Dimensions::scalable(0.9, 0.9));
    let mut rng = TestRng(5);
    let edge = f64::from(PEN) - 0.45;
    for index in 0..count {
        let position = DVec3::new(
            rng.range(-edge, edge),
            f64::from(FLOOR),
            rng.range(-edge, edge),
        );
        let body = Body {
            position,
            ..Body::default()
        };
        let key = EntityKey(u128::from(index));
        let state = EntityState::new(key, body, Some(Living::of(&motion)));
        scene.rows.spawn(EntityId(u64::from(index)), state, motion);
    }
    scene
}

#[test]
fn a_dense_crowd_with_room_to_spread_comes_to_rest() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = pen();
    for _ in 0..TICKS {
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
    let mut rows = 0;
    let mut resting = 0;
    for (_, state) in scene.rows.living_rows() {
        rows += 1;
        resting += usize::from(matches!(state.rest, Rest::Resting(_)));
    }
    assert!(
        resting * 100 >= rows * 95,
        "{resting} of {rows} bodies rest after {TICKS} ticks"
    );
}
