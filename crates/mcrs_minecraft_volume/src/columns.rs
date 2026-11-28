use mcrs_minecraft_core::HeightmapName;
use mcrs_voxel::BlockPos;

use crate::access::{Unavailable, Volume};

pub trait HeightVolume: Volume {
    fn heightmap(&self, kind: HeightmapName, x: i32, z: i32) -> Result<i32, Unavailable>;
}

pub trait BiomeVolume: Volume {
    fn biome_at(&self, at: BlockPos) -> Result<u16, Unavailable>;
}
