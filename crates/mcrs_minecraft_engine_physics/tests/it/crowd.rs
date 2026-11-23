#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_pose::Dimensions;
use mcrs_voxel::BlockPos;

const PEN: i32 = 14;
const DENSITY: f64 = 0.8;
const TICKS: u32 = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpawnOrder {
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug)]
struct Strike {
    uuid: EntityUuid,
    died: bool,
}

#[derive(Resource, Default)]
struct Strikes(Vec<Strike>);

fn record_strikes(
    mut damaged: MessageReader<Damaged>,
    states: Query<&MotionState>,
    mut strikes: ResMut<Strikes>,
) {
    strikes.0.clear();
    for hit in damaged.read() {
        if let Ok(state) = states.get(hit.target) {
            strikes.0.push(Strike {
                uuid: state.0.key,
                died: hit.report.died,
            });
        }
    }
}

fn scene() -> TestScene {
    let mut scene = TestScene::new(1, 0, 0, 3);
    scene.app.init_resource::<Strikes>();
    scene
        .app
        .add_systems(FixedUpdate, record_strikes.in_set(DamageSet::Report));
    scene
}

fn strikes(scene: &TestScene) -> &[Strike] {
    &scene.app.world().resource::<Strikes>().0
}

fn wall(scene: &mut TestScene, at: BlockPos) {
    let stone = TestBlocks::new().stone;
    scene
        .set_block(at, stone)
        .expect("the wall is inside the floor");
}

fn pen() -> TestScene {
    let mut scene = scene();
    let mut rules = GameRules::default();
    rules.set(GameRule::MaxEntityCramming, 0);
    scene.app.insert_resource(rules);
    for along in -PEN - 1..=PEN {
        for y in FLOOR..FLOOR + 2 {
            for at in [
                BlockPos::new(along, y, -PEN - 1),
                BlockPos::new(along, y, PEN),
                BlockPos::new(-PEN - 1, y, along),
                BlockPos::new(PEN, y, along),
            ] {
                wall(&mut scene, at);
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
        let key = EntityUuid::from_u128(u128::from(index));
        let state = EntityState::new(key, body, Some(Living::of(&motion)));
        scene.spawn(EntityId(u64::from(index)), state, motion);
    }
    scene
}

#[test]
fn a_dense_crowd_with_room_to_spread_comes_to_rest() {
    let mut scene = pen();
    let spawned = scene.states().len();
    for _ in 0..TICKS {
        scene.step();
    }
    let states = scene.states();
    let living = states.iter().filter(|(_, state)| state.living.is_some());
    let rows = living.clone().count();
    let resting = living
        .filter(|(_, state)| matches!(state.rest, Rest::Resting(_)))
        .count();
    assert_eq!(rows, spawned, "every walker stays in the pen");
    assert!(
        resting * 100 >= rows * 95,
        "{resting} of {rows} bodies rest after {TICKS} ticks"
    );
}

const CRAMMING: u32 = 7;

fn add_pig(scene: &mut TestScene, index: u32, position: DVec3, health: f32) -> Entity {
    let motion = MotionRow::living(Category::Walker, Dimensions::scalable(0.9, 0.9));
    let living = Living::of(&motion);
    let body = Body {
        position,
        ..Body::default()
    };
    let state = EntityState::new(EntityUuid::from_u128(u128::from(index)), body, Some(living));
    scene.spawn_with_vitals(
        EntityId(u64::from(index)),
        state,
        motion,
        Vitals::new(health),
    )
}

fn alive(scene: &mut TestScene) -> usize {
    let world = scene.app.world_mut();
    let mut query = world.query::<(&MotionState, &Vitals)>();
    query
        .iter(world)
        .filter(|(state, vitals)| state.0.living.is_some() && vitals.is_alive())
        .count()
}

#[test]
fn a_stack_at_one_point_spreads_once_one_body_moves() {
    let mut scene = scene();
    let at = DVec3::new(0.5, f64::from(FLOOR), 0.5);
    let pigs: Vec<_> = (0..65)
        .map(|index| add_pig(&mut scene, index, at, 10.0))
        .collect();
    let walker = *pigs.get(25).expect("the stack has 65 pigs");
    scene
        .app
        .world_mut()
        .get_mut::<MotionState>(walker)
        .expect("the pig moves")
        .0
        .body
        .velocity = DVec3::new(-0.05, 0.0, 0.05);
    for _ in 0..TICKS {
        scene.step();
    }
    let positions: Vec<DVec3> = scene
        .states()
        .iter()
        .filter(|(_, state)| state.living.is_some())
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

fn crowd_in_hole(first: i32, last: i32, count: u32, health: f32, order: SpawnOrder) -> TestScene {
    let mut scene = scene();
    scene.set_crowd_damage_type(Some(CRAMMING));
    for x in first - 1..=last + 1 {
        for z in -1..=1 {
            if z == 0 && (first..=last).contains(&x) {
                continue;
            }
            for y in FLOOR..FLOOR + 2 {
                wall(&mut scene, BlockPos::new(x, y, z));
            }
        }
    }
    let mut rng = TestRng(11);
    let middle = f64::from(first + last + 1) / 2.0;
    let reach = f64::from(last - first) * 0.4 + 0.04;
    let mut pigs: Vec<(u32, DVec3)> = (0..count)
        .map(|index| {
            let position = DVec3::new(
                middle + rng.range(-reach, reach),
                f64::from(FLOOR),
                0.5 + rng.range(-0.04, 0.04),
            );
            (index, position)
        })
        .collect();
    if order == SpawnOrder::Reverse {
        pigs.reverse();
    }
    for (index, position) in pigs {
        add_pig(&mut scene, index, position, health);
    }
    scene
}

#[test]
fn crw_002_a_crowd_of_30_loses_exactly_6() {
    let mut scene = crowd_in_hole(0, 0, 30, 5.0, SpawnOrder::Forward);
    scene.tick = 1;
    scene.step();
    let hits = strikes(&scene);
    assert_eq!(hits.len(), 6);
    assert!(hits.iter().all(|hit| hit.died));
    for _ in 1..1200 {
        scene.step();
        assert!(strikes(&scene).is_empty());
    }
    assert_eq!(alive(&mut scene), 24);
    let living = scene.states().len();
    assert_eq!(living, 24, "the dead mobs left");
}

fn strike_the_first_pig_outside_the_crowd(
    states: Query<&MotionState>,
    mut pending: ResMut<PendingHits>,
) {
    let first = (pending.requests().iter())
        .filter_map(|request| Some((states.get(request.target).ok()?.0.key, request)))
        .min_by_key(|(key, _)| *key)
        .map(|(_, request)| (request.target, request.tick));
    let Some((victim, tick)) = first else {
        return;
    };
    pending.retain(|request| request.target != victim);
    let hit = Hit::new(100.0, DamageSource::of(CRAMMING));
    pending.push(DamageRequest::new(
        victim,
        hit,
        WriterKey::Command { sequence: 0 },
        tick,
        0,
    ));
}

const VICTIM: EntityUuid = EntityUuid::from_u128(2);

#[test]
fn crw_002_a_death_by_another_hit_relieves_the_crowd() {
    for launch in [0.0, 2.0] {
        let mut scene = crowd_in_hole(0, 0, 25, 5.0, SpawnOrder::Forward);
        scene.app.add_systems(
            FixedUpdate,
            strike_the_first_pig_outside_the_crowd.in_set(DamageSet::Filter),
        );
        let world = scene.app.world_mut();
        let mut states = world.query::<&mut MotionState>();
        for mut state in states.iter_mut(world) {
            if state.0.key == VICTIM {
                state.0.body.velocity.y = launch;
            }
        }
        scene.tick = 1;
        scene.step();
        let hits: Vec<(EntityUuid, bool)> = strikes(&scene)
            .iter()
            .map(|hit| (hit.uuid, hit.died))
            .collect();
        assert_eq!(
            hits,
            [(VICTIM, true)],
            "24 pigs are left after the first death, so no crowd hit lands; the victim leaves the crowd at {launch} blocks per tick"
        );
    }
}

#[test]
fn psh_007_a_crowd_of_25_loses_one_and_a_crowd_of_24_takes_no_damage() {
    let mut scene = crowd_in_hole(0, 0, 25, 10.0, SpawnOrder::Forward);
    let mut first_death = None;
    for tick in 1..=1200 {
        scene.step();
        if first_death.is_none() && alive(&mut scene) < 25 {
            first_death = Some(tick);
        }
    }
    assert_eq!(alive(&mut scene), 24);
    assert!(
        first_death.is_some_and(|tick| tick >= 11),
        "two hits are at least 10 ticks apart: {first_death:?}"
    );

    let mut scene = crowd_in_hole(0, 0, 24, 10.0, SpawnOrder::Forward);
    for _ in 0..1200 {
        scene.step();
        assert!(strikes(&scene).is_empty());
    }
    assert_eq!(alive(&mut scene), 24);
}

#[test]
fn crw_002_the_crowd_hits_do_not_depend_on_the_spawn_order() {
    let run = |order: SpawnOrder| {
        let mut scene = crowd_in_hole(-1, 0, 40, 5.0, order);
        for _ in 0..30 {
            scene.step();
        }
        let states: Vec<EntityState> = scene.states().into_iter().map(|(_, state)| state).collect();
        (alive(&mut scene), format!("{states:?}"))
    };
    let (left, expected) = run(SpawnOrder::Forward);
    assert!(left < 40, "the crowd takes damage");
    let mut scene = crowd_in_hole(-1, 0, 40, 5.0, SpawnOrder::Reverse);
    scene.tick = 1;
    scene.step();
    let hit: Vec<EntityUuid> = strikes(&scene).iter().map(|hit| hit.uuid).collect();
    assert!(
        hit.len() > 2 && hit.is_sorted(),
        "the hits are in key order: {hit:?}"
    );
    assert!(
        run(SpawnOrder::Reverse).1 == expected,
        "the reverse spawn order gives the same state"
    );
}
