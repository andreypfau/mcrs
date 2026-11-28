use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::BuildHeight;
use crate::GroundCell;
use crate::straight_route;

pub trait GoalWorld {
    fn build_height(&self) -> BuildHeight;

    fn cell(&self, at: BlockPos) -> GroundCell;

    fn line_of_sight(&self, from: DVec3, to: DVec3) -> bool;

    fn route(&self, from: DVec3, target: BlockPos) -> Option<DVec3> {
        straight_route::straight_route(self, from, target)
    }
}
