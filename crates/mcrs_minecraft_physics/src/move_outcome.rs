use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::StepUp;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveOutcome {
    pub requested: DVec3,
    pub resolved: DVec3,
    pub step: Option<StepUp>,
    pub clipped: ClippedAxes,
    pub bounce: Option<BlockPos>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClippedAxes {
    pub x: bool,
    pub z: bool,
}
