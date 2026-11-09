use glam::DVec3;
use mcrs_minecraft_brain::BlockPos;

use crate::BuildHeight;
use crate::CellFlags;
use crate::GoalWorld;
use crate::GroundCell;
use crate::straight_route;

/// The malus of water for a mob that does not swim.
const WATER_MALUS: f32 = 8.0;

/// A grass floor at `y` = 0 on the square from 0 to `side` on the `x` and `z` axes, with stone
/// below and air above; around the square, solid blocks with no floor. A pond replaces the grass
/// with water.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlatGround {
    pub side: i32,
    pub pond: Option<Pond>,
}

/// Water at `y` = 0 on the blocks from `min_x` to `max_x` and from `min_z` to `max_z`, all
/// inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pond {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl Pond {
    #[must_use]
    pub const fn holds(&self, at: BlockPos) -> bool {
        at.y == 0
            && self.min_x <= at.x
            && at.x <= self.max_x
            && self.min_z <= at.z
            && at.z <= self.max_z
    }
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
            0 if self.pond.is_some_and(|pond| pond.holds(at)) => GroundCell {
                flags: CellFlags::STABLE_FLOOR
                    .union(CellFlags::NO_COLLISION)
                    .union(CellFlags::WATER),
                malus: WATER_MALUS,
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

    /// The straight route wades through the pond, as a path search crosses water at its malus.
    fn route(&self, from: DVec3, target: BlockPos) -> Option<DVec3> {
        match self.pond {
            Some(_) => straight_route::straight_route(&Wading(self), from, target),
            None => straight_route::straight_route(self, from, target),
        }
    }
}

/// A [`FlatGround`] whose water has no malus.
struct Wading<'a>(&'a FlatGround);

impl GoalWorld for Wading<'_> {
    fn build_height(&self) -> BuildHeight {
        self.0.build_height()
    }

    fn cell(&self, at: BlockPos) -> GroundCell {
        let cell = self.0.cell(at);
        if cell.flags.contains(CellFlags::WATER) {
            GroundCell { malus: 0.0, ..cell }
        } else {
            cell
        }
    }

    fn line_of_sight(&self, from: DVec3, to: DVec3) -> bool {
        self.0.line_of_sight(from, to)
    }
}
