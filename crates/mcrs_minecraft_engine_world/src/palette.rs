use mcrs_minecraft_chunk::PalettedContainer::{Heterogeneous, Homogeneous};
pub use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_chunk::{SharedVoxelPalette, VoxelPalette};
use mcrs_minecraft_core::{BlockStateId, SectionPos};

/// One section's block states as the engine keeps them.
pub type SectionStates = VoxelPalette<BlockStateId, { SectionPos::SIZE }>;

/// The blocks a loaded chunk entity holds: the engine's shared section
/// palette, named in this crate's vocabulary.
pub type ChunkBlocks = SharedVoxelPalette<BlockStateId, { SectionPos::SIZE }>;

/// The states of a section that worldgen or a save produced, checked against the
/// `state_count` states of the block table. An id past the table reads as air.
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

// Coupling: this assumes `VoxelId(0)` is the air state, which
// is the current vanilla convention but is not enforced by `BlockPalette`
// itself. Reordering the static block registry so air ends up at a
// different ID would silently break this count. The principled fix is to
// consult the `IS_NOT_AIR` bit from `BlockStateLightTable.flags_for`, but that
// requires plumbing the table through `non_air_block_count`'s callers in
// `column_view`. Tracked as a follow-up.
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
