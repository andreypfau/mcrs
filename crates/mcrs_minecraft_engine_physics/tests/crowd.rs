//! A crowd that has room to spread settles: the push moves the bodies apart until no box
//! overlaps another, and then they rest (PHY-PSH-006, PHY-CMP-012, PHY-SKIP-006). A stack at one
//! point spreads in the same way (PHY-CMP-027). A crowd with no room takes the crowd damage
//! until it is small enough (PHY-PSH-007, DMG-CRW-002).

#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
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
    let mut scene = TestScene::<()>::new(1, 0, 0, 3);
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

/// A stand-in for the network id of the damage type `cramming`.
const CRAMMING: u32 = 7;

/// Adds an idle pig with `health`, and `index` as its key and its id.
fn add_pig(scene: &mut TestScene, index: u32, position: DVec3, health: f32) {
    let motion = MotionRow::living(Category::Walker, Dimensions::scalable(0.9, 0.9));
    let mut living = Living::of(&motion);
    living.health = health;
    let body = Body {
        position,
        ..Body::default()
    };
    let state = EntityState::new(EntityKey(u128::from(index)), body, Some(living));
    scene.rows.spawn(EntityId(u64::from(index)), state, motion);
}

fn alive(scene: &TestScene) -> usize {
    scene
        .rows
        .living_rows()
        .filter(|(_, state)| state.living.is_some_and(|living| living.is_alive()))
        .count()
}

/// E55: 65 pigs at one point give each other no push. When one of them moves, the stack spreads
/// until no box overlaps another.
#[test]
fn a_stack_at_one_point_spreads_once_one_body_moves() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = TestScene::<()>::new(1, 0, 0, 3);
    let at = DVec3::new(0.5, f64::from(FLOOR), 0.5);
    for index in 0..65 {
        add_pig(&mut scene, index, at, 10.0);
    }
    let walker = Placed {
        tile: TileKey::at(at),
        id: EntityId(25),
    };
    scene
        .rows
        .state_mut(walker)
        .expect("the pig is in the tile of the stack")
        .body
        .velocity = DVec3::new(-0.05, 0.0, 0.05);
    for _ in 0..TICKS {
        scene.step(&pool, 1, &mut EntityWrites::default());
    }
    let positions: Vec<DVec3> = scene
        .rows
        .living_rows()
        .map(|(_, state)| state.body.position)
        .collect();
    let width = 2.0 * f64::from(0.45_f32);
    let overlaps = positions
        .iter()
        .enumerate()
        .flat_map(|(index, a)| positions.iter().skip(index + 1).map(move |b| *a - *b))
        .filter(|offset| offset.x.abs() < width && offset.z.abs() < width)
        .count();
    assert_eq!(
        overlaps, 0,
        "pairs of boxes that overlap after {TICKS} ticks"
    );
}

/// An empty scene with the crowd damage and a hole one block deep and two blocks high over the
/// blocks from `(first, FLOOR, 0)` to `(last, FLOOR, 0)`, with `count` pigs of `health` in it.
/// Each box overlaps each other box.
fn crowd_in_hole(first: i32, last: i32, count: u32, health: f32) -> TestScene {
    let mut scene = TestScene::<()>::new(1, 0, 0, 3);
    scene.crowd_damage_type = Some(CRAMMING);
    let stone = TestBlocks::new().stone;
    for x in first - 1..=last + 1 {
        for z in -1..=1 {
            if z == 0 && (first..=last).contains(&x) {
                continue;
            }
            for y in FLOOR..FLOOR + 2 {
                scene
                    .blocks
                    .set_block(BlockPos::new(x, y, z), stone)
                    .expect("the wall is inside the floor");
            }
        }
    }
    let mut rng = TestRng(11);
    let middle = f64::from(first + last + 1) / 2.0;
    let reach = f64::from(last - first) * 0.4 + 0.04;
    for index in 0..count {
        let position = DVec3::new(
            middle + rng.range(-reach, reach),
            f64::from(FLOOR),
            0.5 + rng.range(-0.04, 0.04),
        );
        add_pig(&mut scene, index, position, health);
    }
    scene
}

/// DMG-CRW-002, D7: in the first tick 13 of the 30 draw the crowd damage, and each hit kills.
/// The deaths of the entities with lower keys leave the counts of the later ones, so exactly 6
/// die, and the 24 that stay take no more damage.
#[test]
fn crw_002_a_crowd_of_30_loses_exactly_6() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = crowd_in_hole(0, 0, 30, 5.0);
    scene.tick = 1;
    scene.step(&pool, 1, &mut EntityWrites::default());
    let hits = scene.rows.crowd_hits();
    assert_eq!(hits.len(), 6);
    assert!(hits.iter().all(|hit| hit.died));
    for _ in 1..1200 {
        scene.step(&pool, 1, &mut EntityWrites::default());
        assert!(scene.rows.crowd_hits().is_empty());
    }
    assert_eq!(alive(&scene), 24);
    assert_eq!(scene.rows.living_rows().count(), 24, "the dead rows left");
}

/// PHY-PSH-007, DMG-CRW-003: 25 pigs in a hole of one block begin to die, the first one not
/// before its second hit, and 24 never do.
#[test]
fn psh_007_a_crowd_of_25_loses_one_and_a_crowd_of_24_takes_no_damage() {
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = crowd_in_hole(0, 0, 25, 10.0);
    let mut first_death = None;
    for tick in 1..=1200 {
        scene.step(&pool, 1, &mut EntityWrites::default());
        if first_death.is_none() && alive(&scene) < 25 {
            first_death = Some(tick);
        }
    }
    assert_eq!(alive(&scene), 24);
    assert!(
        first_death.is_some_and(|tick| tick >= 11),
        "two hits are at least 10 ticks apart: {first_death:?}"
    );

    let mut scene = crowd_in_hole(0, 0, 24, 10.0);
    for _ in 0..1200 {
        scene.step(&pool, 1, &mut EntityWrites::default());
        assert!(scene.rows.crowd_hits().is_empty());
    }
    assert_eq!(alive(&scene), 24);
}

/// DMG-CRW-002, PHY-CMP-013: the crowd hits follow the order of the keys, so a crowd over two
/// tiles loses the same entities for each thread count and task split.
#[test]
fn crw_002_the_crowd_hits_do_not_depend_on_the_task_split() {
    let run = |threads: usize, tiles_per_task: usize| {
        let pool = TaskPoolBuilder::new().num_threads(threads).build();
        let mut scene = crowd_in_hole(-1, 0, 40, 5.0);
        for _ in 0..30 {
            scene.step(&pool, tiles_per_task, &mut EntityWrites::default());
        }
        (alive(&scene), format!("{:?}", scene.rows))
    };
    let (left, expected) = run(1, 1);
    assert!(left < 40, "the crowd takes damage");
    let pool = TaskPoolBuilder::new().num_threads(2).build();
    let mut scene = crowd_in_hole(-1, 0, 40, 5.0);
    scene.tick = 1;
    scene.step(&pool, 1, &mut EntityWrites::default());
    let hit: Vec<u64> = scene
        .rows
        .crowd_hits()
        .iter()
        .map(|hit| hit.at.id.0)
        .collect();
    assert!(
        hit.len() > 2 && hit.is_sorted(),
        "the hits are in key order: {hit:?}"
    );
    for (threads, tiles_per_task) in [(4, 1), (2, 100)] {
        assert!(
            run(threads, tiles_per_task).1 == expected,
            "{threads} threads, {tiles_per_task} tiles per task"
        );
    }
}
