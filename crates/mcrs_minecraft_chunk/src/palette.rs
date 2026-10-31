use crate::section::{Biomes, Blocks};
use crate::{SectionKind, VoxelId, VoxelPalette};
use mcrs_voxel::SectionPos;

pub type BlockPalette = VoxelPalette<VoxelId, { SectionPos::SIZE }>;
pub type BiomePalette = VoxelPalette<u8, { SectionPos::SIZE }>;

// A container whose edge length disagrees with its section kind's axis bits packs
// to a wrong length at runtime instead of failing to compile.
const _: () = assert!(BlockPalette::SIZE == 1 << Blocks::AXIS_BITS);
const _: () = assert!(BiomePalette::SIZE == 1 << Biomes::AXIS_BITS);
