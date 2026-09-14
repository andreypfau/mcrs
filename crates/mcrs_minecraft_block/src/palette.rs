use mcrs_minecraft_chunk::PalettedContainer::{Heterogeneous, Homogeneous};
use mcrs_minecraft_chunk::section::{Biomes, Blocks};
use mcrs_minecraft_chunk::{SectionKind, SharedVoxelPalette, VoxelId, VoxelPalette};
use mcrs_minecraft_core::SectionPos;

pub type BlockPalette = VoxelPalette<VoxelId, { SectionPos::SIZE }>;
pub type BiomePalette = VoxelPalette<u8, 4>;

/// The blocks a loaded chunk entity holds: the engine's shared section
/// palette, named in this crate's vocabulary.
pub type ChunkBlocks = SharedVoxelPalette<VoxelId, { SectionPos::SIZE }>;

// A container whose edge length disagrees with its section kind's axis bits packs
// to a wrong length at runtime instead of failing to compile.
const _: () = assert!(BlockPalette::SIZE == 1 << Blocks::AXIS_BITS);
const _: () = assert!(BiomePalette::SIZE == 1 << Biomes::AXIS_BITS);

pub trait AirCount {
    // Coupling: this method assumes `VoxelId(0)` is the air state, which
    // is the current vanilla convention but is not enforced by `BlockPalette`
    // itself. Reordering the static block registry so air ends up at a
    // different ID would silently break this count. The principled fix is to
    // consult the `IS_NOT_AIR` bit from `BlockStateLightTable.flags_for`, but that
    // requires plumbing the table through `non_air_block_count`'s callers in
    // `column_view`. Tracked as a follow-up.
    fn non_air_block_count(&self) -> u16;
}

impl AirCount for BlockPalette {
    fn non_air_block_count(&self) -> u16 {
        match &self.0 {
            Homogeneous(registry_id) => {
                if registry_id.0 != 0 {
                    SectionPos::VOLUME as u16
                } else {
                    0
                }
            }
            Heterogeneous(data) => data
                .palette
                .iter()
                .zip(data.counts.iter())
                .filter_map(|(registry_id, count)| {
                    if registry_id.0 != 0 {
                        Some(*count)
                    } else {
                        None
                    }
                })
                .sum(),
        }
    }
}
