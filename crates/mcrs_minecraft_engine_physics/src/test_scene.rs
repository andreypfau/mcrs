use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::Pass;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Steering;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_minecraft_volume::SectionBlocksMut;
use mcrs_minecraft_volume::SparseVolume;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::EntityRows;
use crate::EntityWrites;
use crate::block_world::BlockWorld;

/// The top of the stone floor of a scene: inside a section, so the floor section has two states.
pub const FLOOR: i32 = 4;

/// A square of tiles around the origin with a stone floor, walkers on it and items above it, for
/// tests and benchmarks of the pass.
pub struct TestScene {
    pub blocks: SparseVolume,
    pub tables: Tables,
    pub rows: EntityRows,
    pub grid: BodyGrid,
    pub tick: u64,
    pub push: PushRules,
}

impl TestScene {
    /// `half` tiles on each side of the origin; half of the walkers walk, the others stand.
    #[must_use]
    pub fn new(half: i32, walkers: u32, items: u32, seed: u64) -> Self {
        let test_blocks = TestBlocks::new();
        let edge = half * 32;
        let mut blocks = SparseVolume::unbounded();
        for z in (-edge / 16)..(edge / 16) {
            for x in (-edge / 16)..(edge / 16) {
                blocks.insert(SectionPos::new(x, -1, z), test_blocks.stone);
                blocks.insert(SectionPos::new(x, 0, z), BlockStateId::AIR);
                blocks.insert(SectionPos::new(x, 1, z), BlockStateId::AIR);
                if let Ok(mut section) = blocks.section_mut(SectionPos::new(x, 0, z)) {
                    let top = u8::try_from(FLOOR - 1).unwrap_or_default();
                    section.fill_box(
                        LocalPos::new(0, 0, 0),
                        LocalPos::new(15, top, 15),
                        test_blocks.stone,
                    );
                }
            }
        }
        let mut rng = TestRng(seed);
        let mut rows = EntityRows::default();
        let range = f64::from(edge) - 1.0;
        for index in 0..walkers.saturating_add(items) {
            let key = EntityKey(u128::from(index));
            let x = rng.range(-range, range);
            let z = rng.range(-range, range);
            let walker = index < walkers;
            let (motion, body, living) = if walker {
                let motion = MotionRow::zombie();
                let mut living = Living::of(&motion);
                living.yaw = rng.range(0.0, 360.0) as f32;
                if index % 2 == 0 {
                    living.steering = Some(Steering {
                        input: MoveInput {
                            forward: 1.0,
                            ..MoveInput::default()
                        },
                        jump: false,
                        speed: 0.1,
                    });
                }
                let position = DVec3::new(x, f64::from(FLOOR), z);
                (
                    motion,
                    Body {
                        position,
                        ..Body::default()
                    },
                    Some(living),
                )
            } else {
                let position = DVec3::new(x, f64::from(FLOOR) + rng.range(0.0, 8.0), z);
                let velocity = DVec3::new(rng.range(-0.1, 0.1), 0.2, rng.range(-0.1, 0.1));
                let body = Body {
                    position,
                    velocity,
                    ..Body::default()
                };
                (MotionRow::item(), body, None)
            };
            let mut state = EntityState::new(key, body, living);
            state.phase = (index % 4) as u8;
            rows.spawn(EntityId(u64::from(index)), state, motion);
        }
        Self {
            blocks,
            tables: Tables::new(test_blocks.table),
            rows,
            grid: BodyGrid::default(),
            tick: 0,
            push: PushRules::default(),
        }
    }

    /// One tick: the merge point, the freeze, the pass and the tile moves.
    pub fn step(&mut self, pool: &TaskPool, tiles_per_task: usize, writes: &mut EntityWrites) {
        self.rows.apply_writes(&mut writes.0);
        self.rows.freeze(pool, &mut self.grid);
        let pass = Pass {
            world: &BlockWorld {
                blocks: &self.blocks,
                table: &self.tables.collision,
            },
            tables: &self.tables,
            tick: self.tick,
            push: self.push,
            tiles_per_task,
        };
        self.rows.tick(&pass, pool, &self.grid);
        self.rows.move_rows();
        self.tick = self.tick.wrapping_add(1);
    }
}
