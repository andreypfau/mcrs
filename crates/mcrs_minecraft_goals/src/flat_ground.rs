use glam::DVec3;
use mcrs_minecraft_brain::BlockPos;

use crate::BuildHeight;
use crate::CellFlags;
use crate::GoalWorld;
use crate::GroundCell;

/// A grass floor at `y` = 0 on the square from 0 to `side` on the `x` and `z` axes, with stone
/// below and air above; around the square, solid blocks with no floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlatGround {
    pub side: i32,
}

impl GoalWorld for FlatGround {
    fn build_height(&self) -> BuildHeight {
        BuildHeight { min: -64, max: 319 }
    }

    fn cell(&self, at: BlockPos) -> GroundCell {
        let inside = (0..self.side).contains(&at.x) && (0..self.side).contains(&at.z);
        if !inside {
            return GroundCell {
                flags: CellFlags::LEGACY_SOLID,
                malus: -1.0,
                walk_value: 0.0,
            };
        }
        match at.y {
            ..0 => GroundCell {
                flags: CellFlags::LEGACY_SOLID.union(CellFlags::STABLE_FLOOR),
                malus: -1.0,
                walk_value: 0.0,
            },
            0 => GroundCell {
                flags: CellFlags::STABLE_FLOOR.union(CellFlags::NO_COLLISION),
                malus: 0.0,
                walk_value: 10.0,
            },
            _ => GroundCell {
                flags: CellFlags::NO_COLLISION,
                malus: 0.0,
                walk_value: 0.5,
            },
        }
    }

    fn line_of_sight(&self, _from: DVec3, _to: DVec3) -> bool {
        true
    }
}
