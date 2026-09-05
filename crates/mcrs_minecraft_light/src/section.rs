use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{VoxelId, VoxelPalette};

use crate::level::LocalPos;

pub type SectionBlocks = VoxelPalette<VoxelId, { BLOCKS::SIZE }>;

pub fn filled(block: VoxelId) -> SectionBlocks {
    let mut blocks = SectionBlocks::default();
    blocks.fill(block);
    blocks
}

pub fn block_at(blocks: &SectionBlocks, pos: LocalPos) -> VoxelId {
    blocks
        .0
        .get(pos.x() as usize, pos.y() as usize, pos.z() as usize)
}

pub fn set_block(blocks: &mut SectionBlocks, pos: LocalPos, block: VoxelId) {
    blocks
        .0
        .set(pos.x() as usize, pos.y() as usize, pos.z() as usize, block);
}
