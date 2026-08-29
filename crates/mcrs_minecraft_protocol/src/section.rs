use crate::{BlockStateId, VarInt};
use anyhow::Context;
use mcrs_voxel_storage::{SectionKind, ceillog2};

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
    /// `Strategy.globalPaletteBitsInMemory`, the width the wire uses once the
    /// palette outgrows the indirect configurations.
    const DIRECT_BITS: u32;

    /// The form the network format uses, where a palette past
    /// `MAX_INDIRECT_BITS` is dropped in favour of raw registry ids.
    #[inline]
    fn network_form(palette_len: usize) -> PaletteForm {
        match ceillog2(palette_len) {
            0 => PaletteForm::Single,
            bits if bits <= Self::MAX_INDIRECT_BITS => PaletteForm::Indirect {
                bits: bits.max(Self::MIN_INDIRECT_BITS),
            },
            _ => PaletteForm::Direct {
                bits: Self::DIRECT_BITS,
            },
        }
    }

    /// The width the packed longs are stored at for a declared bits-per-entry
    /// byte, which a sender is free to state narrower than the configuration it
    /// selects.
    #[inline]
    fn wire_storage_bits(declared: u8) -> u32 {
        match declared as u32 {
            0 => 0,
            bits if bits <= Self::MIN_INDIRECT_BITS => Self::MIN_INDIRECT_BITS,
            bits if bits <= Self::MAX_INDIRECT_BITS => bits,
            _ => Self::DIRECT_BITS,
        }
    }
}

/// A value a section container holds, which fixes the widths and the entry
/// count its wire form is read at.
pub trait SectionValue: Copy + Into<VarInt> {
    type Section: NetworkSectionKind;

    fn from_registry_id(id: i32) -> anyhow::Result<Self>;
}

impl SectionValue for BlockStateId {
    type Section = Blocks;

    fn from_registry_id(id: i32) -> anyhow::Result<Self> {
        Ok(BlockStateId(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Blocks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Biomes;

impl SectionKind for Blocks {
    const AXIS_BITS: u32 = 4;
    const MIN_INDIRECT_BITS: u32 = 4;
}

impl SectionKind for Biomes {
    const AXIS_BITS: u32 = 2;
    const MIN_INDIRECT_BITS: u32 = 1;
}

impl NetworkSectionKind for Blocks {
    const MAX_INDIRECT_BITS: u32 = 8;
    const DIRECT_BITS: u32 = 15;
}

impl NetworkSectionKind for Biomes {
    const MAX_INDIRECT_BITS: u32 = 3;
    const DIRECT_BITS: u32 = 7;
}

#[cfg(test)]
mod tests {
    use super::PaletteForm::{Direct, Indirect, Single};
    use super::*;

    #[test]
    fn the_width_table_matches_the_strategy_configurations() {
        let blocks = [
            (1, 0, Single),
            (2, 4, Indirect { bits: 4 }),
            (3, 4, Indirect { bits: 4 }),
            (4, 4, Indirect { bits: 4 }),
            (5, 4, Indirect { bits: 4 }),
            (16, 4, Indirect { bits: 4 }),
            (17, 5, Indirect { bits: 5 }),
            (256, 8, Indirect { bits: 8 }),
            (257, 9, Direct { bits: 15 }),
        ];
        for (len, storage, form) in blocks {
            assert_eq!(Blocks::storage_bits(len), storage, "block palette of {len}");
            assert_eq!(Blocks::network_form(len), form, "block palette of {len}");
        }

        let biomes = [
            (1, 0, Single),
            (2, 1, Indirect { bits: 1 }),
            (3, 2, Indirect { bits: 2 }),
            (4, 2, Indirect { bits: 2 }),
            (5, 3, Indirect { bits: 3 }),
            (8, 3, Indirect { bits: 3 }),
            (9, 4, Direct { bits: 7 }),
        ];
        for (len, storage, form) in biomes {
            assert_eq!(Biomes::storage_bits(len), storage, "biome palette of {len}");
            assert_eq!(Biomes::network_form(len), form, "biome palette of {len}");
        }
    }

    #[test]
    fn section_entry_counts_match_the_axis_bits() {
        assert_eq!(Blocks::ENTRY_COUNT, 4096);
        assert_eq!(Biomes::ENTRY_COUNT, 64);
    }
}
