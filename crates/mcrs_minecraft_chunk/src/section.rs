use crate::SectionKind;

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
