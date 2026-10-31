use mcrs_voxel::BlockPos;

use crate::access::{Unavailable, Volume};

/// A volume that keeps light. A cell's light is its block light in the high nibble and
/// its sky light in the low.
pub trait LightVolume: Volume {
    fn light(&self, at: BlockPos) -> Result<u8, Unavailable>;
}
