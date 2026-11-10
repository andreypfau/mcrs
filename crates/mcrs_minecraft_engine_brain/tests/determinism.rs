//! The AI of 2000 pigs over 200 ticks gives the same rows and brains in each tick with a pool of
//! 1, 8 and 16 threads, while the pigs near a player stroll and a cactus hit makes some panic.

use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use glam::DVec3;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_engine_brain::BlockGoalWorld;
use mcrs_minecraft_engine_brain::HotTick;
use mcrs_minecraft_engine_brain::MobAi;
use mcrs_minecraft_engine_brain::MobColumn;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::hurt_mob;
use mcrs_minecraft_engine_physics::mob_body;
use mcrs_minecraft_engine_physics::mob_row;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_protocol_game::entity::MobLooks;

const PIGS: u32 = 2000;
const TICKS: u64 = 200;
const HALF_TILES: i32 = 3;
const CACTUS: u32 = 7;

fn pig_brain(kind: &'static EntityTypeData) -> Option<PigBrain> {
    (kind.entity_type == EntityType::Pig).then(PigBrain::default)
}

struct Run {
    scene: TestScene<MobColumn>,
    mobs: Mobs,
    ai: MobAi<PigBrain>,
    players: Vec<PlayerRow>,
    pool: TaskPool,
}

impl Run {
    fn new(threads: usize) -> Self {
        let mut scene = TestScene::new(HALF_TILES, 0, 0, 1);
        let mut mobs = Mobs::default();
        let edge = f64::from(HALF_TILES * 32 - 2);
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        let mut unit = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for index in 0..PIGS {
            let position = DVec3::new(
                (unit() * 2.0 - 1.0) * edge,
                f64::from(FLOOR),
                (unit() * 2.0 - 1.0) * edge,
            );
            let row = mob_row(&EntityTypeData::PIG);
            let key = EntityKey(u128::from(index));
            let id = EntityId(u64::from(index));
            scene
                .rows
                .spawn(id, mob_body(&EntityTypeData::PIG, &row, key, position), row);
            mobs.0.insert(
                id,
                Mob {
                    kind: &EntityTypeData::PIG,
                    uuid: u128::from(index),
                    looks: MobLooks::default(),
                    custom_name: None,
                    tags: Vec::new(),
                },
            );
        }
        let players = vec![PlayerRow {
            player: EntityId(1),
            key: EntityKey(u128::MAX),
            position: DVec3::new(0.5, f64::from(FLOOR), 0.5),
            eye_height: 1.62,
            flags: PlayerFlags::ALIVE,
            visibility: 1.0,
            vehicle: None,
        }];
        let mut ai = MobAi::new(StageConfig::new(WorldSeed(42)), pig_brain);
        ai.set_panic_causes(PanicCauses::new([CACTUS]));
        Self {
            scene,
            mobs,
            ai,
            players,
            pool: TaskPoolBuilder::new().num_threads(threads).build(),
        }
    }

    fn tick(&mut self, tick: u64) -> u64 {
        if tick == 50 {
            let struck: Vec<Placed> = self
                .scene
                .rows
                .living_rows()
                .map(|(at, _)| at)
                .filter(|at| at.id.0 < 40)
                .collect();
            let mut changes = MobChanges::default();
            for at in struck {
                hurt_mob(
                    &mut self.scene.rows,
                    &mut changes,
                    at,
                    Hit::new(1.0, CACTUS),
                    tick,
                );
            }
        }
        let hot = HotTick {
            blocks: &self.scene.blocks,
            collision: &self.scene.tables.collision,
            mobs: &self.mobs,
        };
        let players = PlayerTable::new(self.players.iter().copied());
        self.ai
            .prepare(&hot, players, &mut self.scene.rows, &self.pool);
        let world = BlockGoalWorld::new(&self.scene.blocks, &self.scene.tables.collision);
        self.ai.run_stage(&self.pool, &world);
        let tiles = self.scene.rows.tile_count();
        let per_task = tiles.div_ceil(self.pool.thread_num() * 4).max(1);
        self.scene
            .step(&self.pool, per_task, &mut EntityWrites::default());
        self.hash()
    }

    fn hash(&mut self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (at, state) in self.scene.rows.living_rows() {
            at.id.hash(&mut hasher);
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
        for (tile, brains) in self.ai.stage().tiles() {
            tile.hash(&mut hasher);
            for entry in brains.iter() {
                entry.mob.hash(&mut hasher);
                entry.wake.hash(&mut hasher);
                entry.brain.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn positions(&self) -> Vec<DVec3> {
        self.scene
            .rows
            .living_rows()
            .map(|(_, state)| state.body.position)
            .collect()
    }
}

#[test]
fn the_ai_of_a_crowd_gives_equal_ticks_at_1_8_and_16_threads() {
    let mut runs: Vec<Run> = [1, 8, 16].into_iter().map(Run::new).collect();
    let start = runs[0].positions();
    for tick in 1..=TICKS {
        let hashes: Vec<u64> = runs.iter_mut().map(|run| run.tick(tick)).collect();
        assert!(
            hashes.iter().all(|hash| *hash == hashes[0]),
            "tick {tick}: the hashes at 1, 8 and 16 threads differ: {hashes:?}"
        );
    }
    let end = runs[0].positions();
    let walked = start
        .iter()
        .zip(&end)
        .filter(|(from, to)| from.with_y(0.0).distance(to.with_y(0.0)) > 1.0)
        .count();
    assert!(
        walked > 20,
        "the pigs near the player and the hit pigs walk: {walked}"
    );
    assert!(runs[0].ai.stage().evaluations() > u64::from(PIGS));
}
