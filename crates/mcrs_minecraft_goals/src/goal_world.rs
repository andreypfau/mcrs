use mcrs_minecraft_brain::BlockPos;
use mcrs_minecraft_brain::Point;

use crate::BuildHeight;
use crate::GroundCell;
use crate::straight_route;

/// The read-only world of the snapshot of an issue tick, as the goals read it. The host implements
/// it on its block copy for one mob type.
pub trait GoalWorld {
    fn build_height(&self) -> BuildHeight;

    fn cell(&self, at: BlockPos) -> GroundCell;

    /// True when a ray from `from` to `to` hits no collision shape of a block (GOAL-QRY-003).
    fn line_of_sight(&self, from: Point, to: Point) -> bool;

    /// The end of a route from `from` to the block `target`, or `None` when the mob cannot reach
    /// it. The provided route is a straight line on walkable ground; a path search overrides it.
    fn route(&self, from: Point, target: BlockPos) -> Option<Point> {
        straight_route::straight_route(self, from, target)
    }
}
