use crate::math::BitSize;
use crate::world::block::BlockPos;
use crate::world::chunk;
use crate::world::chunk::ChunkPos;
use bevy::math::IVec3;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct RegionPos(pub IVec3);

pub type CHUNKS = BitSize<4>;
pub type BLOCKS = BitSize<{ CHUNKS::BITS + chunk::BLOCKS::BITS }>;

impl RegionPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }
}

impl From<ChunkPos> for RegionPos {
    fn from(chunk_pos: ChunkPos) -> Self {
        let region_x = chunk_pos.x >> CHUNKS::BITS;
        let region_y = chunk_pos.y >> CHUNKS::BITS;
        let region_z = chunk_pos.z >> CHUNKS::BITS;
        RegionPos::new(region_x, region_y, region_z)
    }
}

impl From<BlockPos> for RegionPos {
    fn from(block_pos: BlockPos) -> Self {
        let region_x = block_pos.x >> BLOCKS::BITS;
        let region_y = block_pos.y >> BLOCKS::BITS;
        let region_z = block_pos.z >> BLOCKS::BITS;
        RegionPos::new(region_x, region_y, region_z)
    }
}
