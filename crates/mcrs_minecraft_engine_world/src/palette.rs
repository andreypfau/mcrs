use mcrs_minecraft_chunk::PalettedContainer::{Heterogeneous, Homogeneous};
use mcrs_minecraft_chunk::VoxelPalette;
pub use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_voxel::{BlockStateId, SectionPos};

pub type SectionStates = VoxelPalette<BlockStateId, { SectionPos::SIZE }>;

pub use mcrs_minecraft_chunk::ChunkBlocks;

pub fn section_states(blocks: &BlockPalette, state_count: usize) -> SectionStates {
    VoxelPalette(
        blocks
            .0
            .map(|voxel| BlockStateId::checked(voxel.0, state_count).unwrap_or(BlockStateId::AIR)),
    )
}

pub fn air_section() -> ChunkBlocks {
    ChunkBlocks::new(SectionStates::homogeneous(BlockStateId::AIR))
}

pub fn non_air_block_count(blocks: &BlockPalette) -> u16 {
    match &blocks.0 {
        Homogeneous(registry_id) if registry_id.0 != 0 => SectionPos::VOLUME as u16,
        Homogeneous(_) => 0,
        Heterogeneous(data) => data
            .palette
            .iter()
            .zip(data.counts.iter())
            .filter(|(registry_id, _)| registry_id.0 != 0)
            .map(|(_, count)| *count)
            .sum(),
    }
}
