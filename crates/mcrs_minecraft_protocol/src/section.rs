use crate::VarInt;
use anyhow::Context;
use mcrs_minecraft_chunk::{SectionKind, VoxelId, ceillog2};

pub use mcrs_minecraft_chunk::section::{Biomes, Blocks};

pub fn block_direct_bits(block_state_count: usize) -> u32 {
    ceillog2(block_state_count)
}

pub fn biome_direct_bits(registry_len: usize) -> u32 {
    ceillog2(registry_len)
}

/// Which of the palette configurations a container of a given size lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteForm {
    Single,
    /// Values are indices into the palette list.
    Indirect {
        bits: u32,
    },
    /// Values are registry ids and the palette list is absent.
    Direct {
        bits: u32,
    },
}

/// The wire half of `Strategy`: the widths at which a section container reaches
/// the network, which the stored form does not share.
pub trait NetworkSectionKind: SectionKind {
    const MAX_INDIRECT_BITS: u32;

    /// The form the network format uses, where a palette past
    /// `MAX_INDIRECT_BITS` is dropped in favour of raw registry ids packed at
    /// `direct_bits`.
    #[inline]
    fn network_form(palette_len: usize, direct_bits: u32) -> PaletteForm {
        match ceillog2(palette_len) {
            0 => PaletteForm::Single,
            bits if bits <= Self::MAX_INDIRECT_BITS => PaletteForm::Indirect {
                bits: bits.max(Self::MIN_INDIRECT_BITS),
            },
            _ => PaletteForm::Direct { bits: direct_bits },
        }
    }

    /// The width the packed longs are stored at for a declared bits-per-entry
    /// byte, which a sender is free to state narrower than the configuration it
    /// selects.
    #[inline]
    fn wire_storage_bits(declared: u8, direct_bits: u32) -> u32 {
        match declared as u32 {
            0 => 0,
            bits if bits <= Self::MIN_INDIRECT_BITS => Self::MIN_INDIRECT_BITS,
            bits if bits <= Self::MAX_INDIRECT_BITS => bits,
            _ => direct_bits,
        }
    }
}

/// A value a section container holds, which fixes the widths and the entry
/// count its wire form is read at.
pub trait SectionValue: Copy + Into<VarInt> {
    type Section: NetworkSectionKind;

    fn from_registry_id(id: i32) -> anyhow::Result<Self>;
}

impl SectionValue for VoxelId {
    type Section = Blocks;

    fn from_registry_id(id: i32) -> anyhow::Result<Self> {
        Ok(VoxelId(
            u16::try_from(id).with_context(|| format!("block state id {id}"))?,
        ))
    }
}

impl SectionValue for u8 {
    type Section = Biomes;

    fn from_registry_id(id: i32) -> anyhow::Result<Self> {
        u8::try_from(id).with_context(|| format!("biome id {id}"))
    }
}

impl NetworkSectionKind for Blocks {
    const MAX_INDIRECT_BITS: u32 = 8;
}

impl NetworkSectionKind for Biomes {
    const MAX_INDIRECT_BITS: u32 = 8;
}

#[cfg(test)]
mod tests {
    use super::PaletteForm::{Direct, Indirect, Single};
    use super::*;

    #[test]
    fn the_width_table_matches_the_strategy_configurations() {
        let block_states = 40_000;
        let direct = block_direct_bits(block_states);
        let blocks = [
            (1, 0, Single),
            (2, 4, Indirect { bits: 4 }),
            (3, 4, Indirect { bits: 4 }),
            (4, 4, Indirect { bits: 4 }),
            (5, 4, Indirect { bits: 4 }),
            (16, 4, Indirect { bits: 4 }),
            (17, 5, Indirect { bits: 5 }),
            (256, 8, Indirect { bits: 8 }),
            (257, 9, Direct { bits: direct }),
        ];
        for (len, storage, form) in blocks {
            assert_eq!(Blocks::storage_bits(len), storage, "block palette of {len}");
            assert_eq!(
                Blocks::network_form(len, direct),
                form,
                "block palette of {len}"
            );
        }

        let registry_len = 100;
        let direct = biome_direct_bits(registry_len);
        let biomes = [
            (1, 0, Single),
            (2, 1, Indirect { bits: 1 }),
            (3, 2, Indirect { bits: 2 }),
            (4, 2, Indirect { bits: 2 }),
            (5, 3, Indirect { bits: 3 }),
            (8, 3, Indirect { bits: 3 }),
            (9, 4, Indirect { bits: 4 }),
            (16, 4, Indirect { bits: 4 }),
            (17, 5, Indirect { bits: 5 }),
            (256, 8, Indirect { bits: 8 }),
            (257, 9, Direct { bits: direct }),
        ];
        for (len, storage, form) in biomes {
            assert_eq!(Biomes::storage_bits(len), storage, "biome palette of {len}");
            assert_eq!(
                Biomes::network_form(len, direct),
                form,
                "biome palette of {len}"
            );
        }
    }

    #[test]
    fn the_block_direct_width_follows_the_state_count() {
        let widths = [
            (16_384, 14),
            (16_385, 15),
            (32_768, 15),
            (32_769, 16),
            (65_536, 16),
        ];
        for (block_state_count, bits) in widths {
            assert_eq!(
                block_direct_bits(block_state_count),
                bits,
                "{block_state_count} block states"
            );
        }
    }

    #[test]
    fn the_biome_direct_width_follows_the_registry_length() {
        let widths = [
            (0, 0),
            (1, 0),
            (64, 6),
            (65, 7),
            (128, 7),
            (129, 8),
            (256, 8),
            (257, 9),
        ];
        for (registry_len, bits) in widths {
            assert_eq!(
                biome_direct_bits(registry_len),
                bits,
                "registry of {registry_len}"
            );
        }
    }

    #[test]
    fn section_entry_counts_match_the_axis_bits() {
        assert_eq!(Blocks::ENTRY_COUNT, 4096);
        assert_eq!(Biomes::ENTRY_COUNT, 4096);
    }
}
