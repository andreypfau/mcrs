use glam::DVec3;
use mcrs_minecraft_goal_world::BuildHeight;
use mcrs_minecraft_goal_world::CellFlags;
use mcrs_minecraft_goal_world::GoalWorld;
use mcrs_minecraft_goal_world::GroundCell;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::PathType;
use mcrs_minecraft_physics::ShapeClass;
use mcrs_minecraft_physics::StateCollision;
use mcrs_minecraft_physics::sight_clear;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_minecraft_volume::Unavailable;
use mcrs_voxel::BlockPos;

use crate::sight_blocks::SightBlocks;

const GRASS_WALK_VALUE: f32 = 10.0;
const OTHER_WALK_VALUE: f32 = 0.5;
const SIGHT_RANGE: f64 = 128.0;

const UNLOADED: StateCollision = StateCollision {
    class: ShapeClass::FullCube,
    flags: BlockFlags::LEGACY_SOLID,
    path: PathType::Blocked,
    ..StateCollision::AIR
};

#[derive(Debug)]
pub struct BlockGoalWorld<'a, V> {
    blocks: &'a V,
    table: &'a CollisionTable,
    height: BuildHeight,
}

impl<'a, V: BlockVolume> BlockGoalWorld<'a, V> {
    #[must_use]
    pub fn new(blocks: &'a V, table: &'a CollisionTable) -> Self {
        let bounds = blocks.bounds();
        Self {
            blocks,
            table,
            height: BuildHeight {
                min: bounds.min.y,
                max: bounds.max.y,
            },
        }
    }

    fn state(&self, at: BlockPos) -> StateCollision {
        match self
            .blocks
            .block(mcrs_voxel::BlockPos::new(at.x, at.y, at.z))
        {
            Ok(state) => self.table.state(state),
            Err(Unavailable::Outside) => StateCollision::AIR,
            Err(Unavailable::Unloaded | Unavailable::Foreign) => UNLOADED,
        }
    }

    #[must_use]
    pub fn feet_top(&self, at: BlockPos) -> Option<f64> {
        let state = self.state(at);
        if state.flags.intersects(BlockFlags::DOOR | BlockFlags::FENCE) {
            return None;
        }
        let shape = self.table.shape_without_context(state.class)?;
        Some(f64::from(at.y) + shape.top())
    }

    fn block_path_type(
        &self,
        at: BlockPos,
        here: StateCollision,
        below: StateCollision,
    ) -> PathType {
        if here.path != PathType::Open || at.y <= self.height.min {
            return here.path;
        }
        if let Some(path) = PathType::over(below.path) {
            return path;
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let neighbour = self.state(at.offset(dx, dy, dz)).path;
                    if let Some(path) = neighbour.as_neighbour() {
                        return path;
                    }
                }
            }
        }
        PathType::Walkable
    }
}

impl<V: BlockVolume> GoalWorld for BlockGoalWorld<'_, V> {
    fn build_height(&self) -> BuildHeight {
        self.height
    }

    fn cell(&self, at: BlockPos) -> GroundCell {
        let here = self.state(at);
        let below = self.state(at.below());
        let marks = [
            (
                below.flags.contains(BlockFlags::SOLID_RENDER),
                CellFlags::STABLE_FLOOR,
            ),
            (
                here.flags.contains(BlockFlags::LEGACY_SOLID),
                CellFlags::LEGACY_SOLID,
            ),
            (here.flags.contains(BlockFlags::WATER), CellFlags::WATER),
            (
                self.table.shape_without_context(here.class).is_none(),
                CellFlags::NO_COLLISION,
            ),
        ];
        let flags = marks
            .into_iter()
            .filter(|(set, _)| *set)
            .fold(CellFlags::NONE, |flags, (_, flag)| flags.union(flag));
        GroundCell {
            flags,
            malus: self.block_path_type(at, here, below).default_malus(),
            walk_value: if below.flags.contains(BlockFlags::GRASS_BLOCK) {
                GRASS_WALK_VALUE
            } else {
                OTHER_WALK_VALUE
            },
        }
    }

    fn line_of_sight(&self, from: DVec3, to: DVec3) -> bool {
        let start = DVec3::new(from.x, from.y, from.z);
        let end = DVec3::new(to.x, to.y, to.z);
        if (end - start).length_squared() > SIGHT_RANGE * SIGHT_RANGE {
            return false;
        }
        sight_clear(&SightBlocks(self.blocks), self.table, start, end)
    }
}
