use mcrs_voxel::BlockPos;

use crate::access::{Unavailable, Volume};

pub trait LightVolume: Volume {
    fn light(&self, at: BlockPos) -> Result<u8, Unavailable>;
}
