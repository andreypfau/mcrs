use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::StepUp;

/// What one move did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveOutcome {
    /// The requested displacement after the stuck multiplier and the edge back-off.
    pub requested: DVec3,
    pub resolved: DVec3,
    pub step: Option<StepUp>,
    pub clipped: ClippedAxes,
    /// The block at the effect position, when a restitution above zero changed the velocity.
    pub bounce: Option<BlockPos>,
}

/// The horizontal axes on which the move differs from the request by at least `1.0E-5F`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClippedAxes {
    pub x: bool,
    pub z: bool,
}
