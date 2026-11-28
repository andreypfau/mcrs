use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use bevy_ecs::entity::Entity;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_engine_brain::MobAi;
use mcrs_minecraft_engine_brain::MobBrainPlugin;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::mob_body;
use mcrs_minecraft_engine_physics::mob_row;
use mcrs_minecraft_engine_physics::mob_vitals;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol_game::entity::MobLooks;

const PIGS: u32 = 2000;
const TICKS: u64 = 200;
const SHORT_TICKS: u64 = 60;
const HALF_TILES: i32 = 3;
const CACTUS: u32 = 7;
const HIT_TICK: u64 = 50;
const SHORT_GOLDEN: u64 = 12_682_452_769_184_371_115;
const LONG_GOLDEN: u64 = 6_193_400_499_467_747_111;

fn pig_brain(kind: &'static EntityTypeData) -> Option<PigBrain> {
    (kind.entity_type == EntityType::Pig).then(PigBrain::default)
}

fn positions() -> Vec<DVec3> {
    let edge = f64::from(HALF_TILES * 32 - 2);
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut unit = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..PIGS)
        .map(|_| {
            let x = (unit() * 2.0 - 1.0) * edge;
            let z = (unit() * 2.0 - 1.0) * edge;
            DVec3::new(x, f64::from(FLOOR), z)
        })
        .collect()
}

fn use_threads(threads: usize) {
    ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(threads).build());
    assert_eq!(
        ComputeTaskPool::get().thread_num(),
        threads,
        "each test runs in its own process, so it sets the size of the pool"
    );
}

struct Run {
    scene: TestScene,
    struck: Vec<Entity>,
}

impl Run {
    fn new(order: &[u32]) -> Self {
        let mut scene = TestScene::new(HALF_TILES, 0, 0, 1);
        scene
            .app
            .add_plugins(MobBrainPlugin::new(pig_brain, PANIC_CAUSES));
        let mut ai = MobAi::new(StageConfig::new(WorldSeed(42)), pig_brain);
        ai.set_panic_causes(PanicCauses::new([CACTUS]));
        let world = scene.app.world_mut();
        world.insert_resource(ai);
        world.spawn((
            Player,
            Owner(PlayerSession(1)),
            EntityUuid::from_u128(u128::MAX),
            Transform {
                translation: DVec3::new(0.5, f64::from(FLOOR), 0.5),
                ..Transform::IDENTITY
            },
            Pose::Standing,
        ));
        let positions = positions();
        let mut struck = Vec::new();
        for &index in order {
            let Some(&position) = positions.get(index as usize) else {
                continue;
            };
            let row = mob_row(&EntityTypeData::PIG);
            let key = EntityUuid::from_u128(u128::from(index));
            let entity = world
                .spawn((
                    MotionState(mob_body(&EntityTypeData::PIG, &row, key, position)),
                    MotionKind(row),
                    MotionId(EntityId(u64::from(index))),
                    MobIntent::default(),
                    mob_vitals(&EntityTypeData::PIG),
                    HitRecord::default(),
                    Mob {
                        kind: &EntityTypeData::PIG,
                        uuid: u128::from(index),
                        looks: MobLooks::default(),
                        custom_name: None,
                        tags: Vec::new(),
                    },
                ))
                .id();
            if index < 40 {
                struck.push(entity);
            }
        }
        Self { scene, struck }
    }

    fn tick(&mut self, tick: u64) -> u64 {
        if tick + 1 == HIT_TICK {
            let mut pending = self.scene.app.world_mut().resource_mut::<PendingHits>();
            for &entity in &self.struck {
                pending.push(DamageRequest::new(
                    entity,
                    Hit::new(1.0, DamageSource::of(CACTUS)),
                    WriterKey::Command { sequence: 0 },
                    HIT_TICK,
                    0,
                ));
            }
        }
        self.scene.step();
        self.hash()
    }

    fn mobs(&mut self) -> Vec<(EntityId, MotionState)> {
        let world = self.scene.app.world_mut();
        let mut query = world.query::<(&MotionId, &MotionState)>();
        let mut mobs: Vec<(EntityId, MotionState)> = query
            .iter(world)
            .filter(|(_, state)| state.0.living.is_some())
            .map(|(id, state)| (id.0, *state))
            .collect();
        mobs.sort_by_key(|(_, state)| state.0.key);
        mobs
    }

    fn hash(&mut self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (id, state) in self.mobs() {
            id.hash(&mut hasher);
            let state = state.0;
            state
                .body
                .position
                .to_array()
                .map(f64::to_bits)
                .hash(&mut hasher);
            if let Some(living) = state.living {
                [
                    living.orientation.yaw,
                    living.orientation.pitch,
                    living.orientation.head_yaw,
                    living.orientation.body_yaw,
                ]
                .map(f32::to_bits)
                .hash(&mut hasher);
            }
        }
        for (tile, brains) in self.ai().stage().tiles() {
            tile.hash(&mut hasher);
            for entry in brains.iter() {
                entry.mob.hash(&mut hasher);
                entry.wake.hash(&mut hasher);
                entry.brain.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn ai(&self) -> &MobAi<PigBrain> {
        self.scene.app.world().resource::<MobAi<PigBrain>>()
    }

    fn positions(&mut self) -> Vec<DVec3> {
        self.mobs()
            .into_iter()
            .map(|(_, state)| state.0.body.position)
            .collect()
    }
}

struct Crowd {
    hash: u64,
    walked: usize,
    evaluations: u64,
}

fn forward() -> Vec<u32> {
    (0..PIGS).collect()
}

#[expect(
    clippy::indexing_slicing,
    reason = "a test helper reports a failure by panicking"
)]
fn run_orders(ticks: u64, orders: &[Vec<u32>]) -> Crowd {
    let mut runs: Vec<Run> = orders.iter().map(|order| Run::new(order)).collect();
    let start = runs[0].positions();
    let mut hash = 0;
    for tick in 1..=ticks {
        let hashes: Vec<u64> = runs.iter_mut().map(|run| run.tick(tick)).collect();
        assert!(
            hashes.iter().all(|other| *other == hashes[0]),
            "tick {tick}: the hashes of the spawn orders differ: {hashes:?}"
        );
        hash = hashes[0];
    }
    let end = runs[0].positions();
    let walked = start
        .iter()
        .zip(&end)
        .filter(|(from, to)| from.with_y(0.0).distance(to.with_y(0.0)) > 1.0)
        .count();
    Crowd {
        hash,
        walked,
        evaluations: runs[0].ai().stage().evaluations(),
    }
}

fn assert_short_crowd(crowd: &Crowd) {
    assert_eq!(
        crowd.hash, SHORT_GOLDEN,
        "every thread count and spawn order ends on the same state"
    );
    assert!(
        crowd.walked > 0,
        "the pigs near the player and the hit pigs walk: {}",
        crowd.walked
    );
    assert!(
        crowd.evaluations > u64::from(PIGS),
        "every pig is evaluated: {}",
        crowd.evaluations
    );
}

#[test]
fn the_ai_of_a_crowd_ends_on_the_golden_state_at_1_thread() {
    use_threads(1);
    assert_short_crowd(&run_orders(SHORT_TICKS, &[forward()]));
}

#[test]
fn the_ai_of_a_crowd_ends_on_the_golden_state_at_8_threads_in_any_spawn_order() {
    use_threads(8);
    let reversed: Vec<u32> = (0..PIGS).rev().collect();
    let mut interleaved: Vec<u32> = (0..PIGS).filter(|index| index % 2 == 1).collect();
    interleaved.extend((0..PIGS).filter(|index| index % 2 == 0));
    assert_short_crowd(&run_orders(
        SHORT_TICKS,
        &[forward(), reversed, interleaved],
    ));
}

mod exhaustive {
    use super::*;

    fn assert_long_crowd(threads: usize) {
        use_threads(threads);
        let crowd = run_orders(TICKS, &[forward()]);
        assert_eq!(
            crowd.hash, LONG_GOLDEN,
            "every thread count ends on the same state"
        );
        assert!(
            crowd.walked > 20,
            "the pigs near the player and the hit pigs walk: {}",
            crowd.walked
        );
        assert!(
            crowd.evaluations > u64::from(PIGS),
            "every pig is evaluated: {}",
            crowd.evaluations
        );
    }

    #[test]
    fn the_ai_of_a_crowd_ends_on_the_golden_state_at_1_thread_in_a_long_run() {
        assert_long_crowd(1);
    }

    #[test]
    fn the_ai_of_a_crowd_ends_on_the_golden_state_at_8_threads_in_a_long_run() {
        assert_long_crowd(8);
    }

    #[test]
    fn the_ai_of_a_crowd_ends_on_the_golden_state_at_16_threads_in_a_long_run() {
        assert_long_crowd(16);
    }
}
