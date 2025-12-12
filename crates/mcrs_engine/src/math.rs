/// Stores the size that spans a fixed amount of bits
/// For example: 1, 2, 4, 8, 16 and 32 sizes match this description
pub struct BitSize<const BITS: usize>;

impl<const BITS: usize> BitSize<BITS> {
    pub const BITS: usize = BITS;
    pub const SIZE: usize = 1 << BITS;
    pub const AREA: usize = Self::SIZE * Self::SIZE;
    pub const VOLUME: usize = Self::AREA * Self::SIZE;
    pub const HALF_SIZE: usize = Self::SIZE >> 1;
    pub const HALF_AREA: usize = Self::AREA >> 1;
    pub const HALF_VOLUME: usize = Self::VOLUME >> 1;
    pub const DOUBLE_SIZE: usize = Self::SIZE << 1;
    pub const DOUBLE_AREA: usize = Self::AREA << 1;
    pub const DOUBLE_VOLUME: usize = Self::VOLUME << 1;
    pub const DOUBLE_BITS: usize = BITS << 1;
    pub const MASK: usize = Self::SIZE - 1;
}
