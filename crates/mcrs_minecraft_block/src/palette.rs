use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_protocol::section::{Biomes, Blocks, NetworkSectionKind, PaletteForm};
use mcrs_voxel_math::chunk_pos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::PalettedContainer::{Heterogeneous, Homogeneous};
use mcrs_voxel_storage::{SectionKind, VoxelId, VoxelPalette};

pub type BlockPalette = VoxelPalette<VoxelId, { BLOCKS::SIZE }>;
pub type BiomePalette = VoxelPalette<u8, 4>;

// A container whose edge length disagrees with its section kind's axis bits packs
// to a wrong length at runtime instead of failing to compile.
const _: () = assert!(BlockPalette::SIZE == 1 << Blocks::AXIS_BITS);
const _: () = assert!(BiomePalette::SIZE == 1 << Biomes::AXIS_BITS);

// According to the wiki, palette serialization for disk and network is different. Disk
// serialization always uses a palette if greater than one entry. Network serialization packs ids
// directly instead of using a palette above a certain bits-per-entry
pub trait NetworkPalette {
    type Value;

    fn convert_network(&self) -> mcrs_minecraft_protocol::chunk::PalettedContainer<Self::Value>;
}

impl NetworkPalette for BiomePalette {
    type Value = u8;

    fn convert_network(&self) -> mcrs_minecraft_protocol::chunk::PalettedContainer<u8> {
        match &self.0 {
            Homogeneous(registry_id) => mcrs_minecraft_protocol::chunk::PalettedContainer {
                bits_per_entry: 0,
                palette: mcrs_minecraft_protocol::chunk::Palette::Single(*registry_id),
                packed_data: Box::new([]),
            },
            Heterogeneous(data) => {
                match mcrs_minecraft_protocol::section::Biomes::network_form(data.counts.len()) {
                    PaletteForm::Single => {
                        unreachable!("a heterogeneous container has two entries")
                    }
                    PaletteForm::Indirect { bits } => {
                        let (palette, packed) = self.0.to_palette_and_packed_data(bits as u8);
                        mcrs_minecraft_protocol::chunk::PalettedContainer {
                            bits_per_entry: bits as u8,
                            palette: mcrs_minecraft_protocol::chunk::Palette::Indirect(palette),
                            packed_data: packed,
                        }
                    }
                    PaletteForm::Direct { bits } => {
                        let cells = data.cube.as_flattened().as_flattened();
                        mcrs_minecraft_protocol::chunk::PalettedContainer {
                            bits_per_entry: bits as u8,
                            palette: mcrs_minecraft_protocol::chunk::Palette::Direct,
                            packed_data: mcrs_voxel_storage::pack_from(bits, cells, |&id| {
                                id as u32
                            }),
                        }
                    }
                }
            }
        }
    }
}

impl NetworkPalette for BlockPalette {
    type Value = BlockStateId;

    fn convert_network(&self) -> mcrs_minecraft_protocol::chunk::PalettedContainer<BlockStateId> {
        match &self.0 {
            Homogeneous(voxel) => mcrs_minecraft_protocol::chunk::PalettedContainer {
                bits_per_entry: 0,
                palette: mcrs_minecraft_protocol::chunk::Palette::Single(BlockStateId::from(
                    *voxel,
                )),
                packed_data: Box::new([]),
            },
            Heterogeneous(data) => {
                match mcrs_minecraft_protocol::section::Blocks::network_form(data.counts.len()) {
                    PaletteForm::Single => {
                        unreachable!("a heterogeneous container has two entries")
                    }
                    PaletteForm::Indirect { bits } => {
                        let (palette, packed) = self.0.to_palette_and_packed_data(bits as u8);
                        let palette = palette.iter().copied().map(BlockStateId::from).collect();
                        mcrs_minecraft_protocol::chunk::PalettedContainer {
                            bits_per_entry: bits as u8,
                            palette: mcrs_minecraft_protocol::chunk::Palette::Indirect(palette),
                            packed_data: packed,
                        }
                    }
                    PaletteForm::Direct { bits } => {
                        let cells = data.cube.as_flattened().as_flattened();
                        mcrs_minecraft_protocol::chunk::PalettedContainer {
                            bits_per_entry: bits as u8,
                            palette: mcrs_minecraft_protocol::chunk::Palette::Direct,
                            packed_data: mcrs_voxel_storage::pack_from(bits, cells, |id| {
                                id.0 as u32
                            }),
                        }
                    }
                }
            }
        }
    }
}

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
                    chunk_pos::BLOCKS::VOLUME as u16
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
