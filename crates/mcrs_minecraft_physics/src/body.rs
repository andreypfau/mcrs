use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::MoveFlags;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub position: DVec3,
    pub velocity: DVec3,
    pub flags: MoveFlags,
    pub support: Option<BlockPos>,
    pub support_missed: bool,
    pub fall_distance: f64,
    pub stuck: DVec3,
}
