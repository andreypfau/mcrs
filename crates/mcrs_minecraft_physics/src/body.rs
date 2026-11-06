use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::MoveFlags;

/// The motion state of an entity that the move procedure reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub position: DVec3,
    pub velocity: DVec3,
    pub flags: MoveFlags,
    pub support: Option<BlockPos>,
    /// True when the last search for the supporting block found no block.
    pub support_missed: bool,
    pub fall_distance: f64,
    /// The stuck multiplier for the next move.
    pub stuck: DVec3,
}
