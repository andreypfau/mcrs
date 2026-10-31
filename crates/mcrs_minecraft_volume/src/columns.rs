use mcrs_minecraft_core::HeightmapName;
use mcrs_voxel::BlockPos;

use crate::access::{Unavailable, Volume};

/// A volume that keeps column heights. A height is the lowest `y` above every block the
/// heightmap counts, so a column that counts no block answers the floor of the world.
pub trait HeightVolume: Volume {
    /// A store answers [`Unavailable::Unloaded`] for a heightmap it does not keep.
    fn heightmap(&self, kind: HeightmapName, x: i32, z: i32) -> Result<i32, Unavailable>;
}

/// A volume that keeps biomes, one per 4³ cell, as the index its world gives them.
pub trait BiomeVolume: Volume {
    fn biome_at(&self, at: BlockPos) -> Result<u16, Unavailable>;
}
