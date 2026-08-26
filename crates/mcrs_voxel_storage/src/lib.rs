pub mod container;

pub use container::{AbstractCube, HeterogeneousPaletteData, PalettedContainer};

/// An opaque voxel identifier. The engine never interprets it: the game assigns
/// ids when its asset corpus loads, and they are not stable across runs, which
/// is why this type has no serialized form.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct VoxelId(pub u16);

impl From<u16> for VoxelId {
    #[inline]
    fn from(id: u16) -> Self {
        VoxelId(id)
    }
}

impl From<VoxelId> for u16 {
    #[inline]
    fn from(id: VoxelId) -> Self {
        id.0
    }
}

impl std::ops::Deref for VoxelId {
    type Target = u16;

    #[inline]
    fn deref(&self) -> &u16 {
        &self.0
    }
}

/// `Mth.ceillog2`: the width an index into `count` distinct values must have.
/// Answers 0 for a count of 0 or 1, which is what a single-value palette needs.
#[inline]
pub const fn ceillog2(count: usize) -> u32 {
    match count {
        0 | 1 => 0,
        n => usize::BITS - (n - 1).leading_zeros(),
    }
}

/// Which of vanilla's palette configurations a container of a given size lands in.
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

/// Vanilla's `Strategy`: the per-axis index formula and the bits-per-entry table
/// for one kind of section container.
pub trait SectionKind {
    const AXIS_BITS: u32;
    const MIN_INDIRECT_BITS: u32;
    const MAX_INDIRECT_BITS: u32;
    /// `Strategy.globalPaletteBitsInMemory`, the width the wire uses once the
    /// palette outgrows the indirect configurations.
    const DIRECT_BITS: u32;

    const ENTRY_COUNT: usize = 1 << (3 * Self::AXIS_BITS);

    #[inline]
    fn index(x: usize, y: usize, z: usize) -> usize {
        (y << Self::AXIS_BITS | z) << Self::AXIS_BITS | x
    }

    /// The width the save format stores at. Values are indices into the palette
    /// list at every width — `Configuration.Global` widens `bitsInMemory` only.
    #[inline]
    fn storage_bits(palette_len: usize) -> u32 {
        match ceillog2(palette_len) {
            0 => 0,
            bits => bits.max(Self::MIN_INDIRECT_BITS),
        }
    }

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Blocks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Biomes;

impl SectionKind for Blocks {
    const AXIS_BITS: u32 = 4;
    const MIN_INDIRECT_BITS: u32 = 4;
    const MAX_INDIRECT_BITS: u32 = 8;
    const DIRECT_BITS: u32 = 15;
}

impl SectionKind for Biomes {
    const AXIS_BITS: u32 = 2;
    const MIN_INDIRECT_BITS: u32 = 1;
    const MAX_INDIRECT_BITS: u32 = 3;
    const DIRECT_BITS: u32 = 7;
}

/// Resolves a value to its registry id. The only place vanilla's `IdMap<T>`
/// reaches the palette layer is `read`/`write`, which is why nothing else here
/// takes one.
pub trait IdMap<T: ?Sized> {
    fn id_of(&self, value: &T) -> Option<u32>;
}

#[inline]
pub const fn entries_per_long(bits: u32) -> usize {
    64 / bits as usize
}

#[inline]
pub const fn packed_len(bits: u32, entry_count: usize) -> usize {
    entry_count.div_ceil(entries_per_long(bits))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataLength {
    pub found: usize,
    pub expected: usize,
}

pub fn check_len(bits: u32, data: &[i64], entry_count: usize) -> Result<(), DataLength> {
    let expected = packed_len(bits, entry_count);
    if data.len() == expected {
        Ok(())
    } else {
        Err(DataLength {
            found: data.len(),
            expected,
        })
    }
}

/// Unpacks `data` into one entry per element of `out`, low entry first, with the
/// spare high bits of each word discarded. `bits` must be in `1..=16`.
pub fn unpack_into(bits: u32, data: &[i64], out: &mut [u16]) -> Result<(), DataLength> {
    debug_assert!((1..=16).contains(&bits));
    check_len(bits, data, out.len())?;
    let per_long = entries_per_long(bits);
    let mask = (1u64 << bits) - 1;
    for (cells, &word) in out.chunks_mut(per_long).zip(data) {
        let mut word = word as u64;
        for cell in cells {
            *cell = (word & mask) as u16;
            word >>= bits;
        }
    }
    Ok(())
}

#[inline]
pub fn entry_at(bits: u32, data: &[i64], index: usize) -> u16 {
    let per_long = entries_per_long(bits);
    let word = data[index / per_long] as u64;
    ((word >> ((index % per_long) as u32 * bits)) & ((1u64 << bits) - 1)) as u16
}

/// Masking alternate lanes leaves `bits` spare bits above each one kept, which
/// is the room a carry out of that lane needs.
fn even_lanes(bits: u32, lane_limit: usize, limit: u64) -> (u64, u64, u64) {
    let lane_mask = (1u64 << bits) - 1;
    let bump = (1u64 << bits) - limit;
    let (mut lanes, mut carries, mut addend) = (0u64, 0u64, 0u64);
    let mut lane = 0usize;
    while lane < lane_limit && (lane as u32 + 1) * bits <= 64 {
        let shift = lane as u32 * bits;
        lanes |= lane_mask << shift;
        carries |= 1u64 << (shift + bits);
        addend |= bump << shift;
        lane += 2;
    }
    (lanes, carries, addend)
}

/// Adding `2^bits - limit` to a lane carries out of it exactly when the lane
/// reaches `limit`, so one word answers for all of its cells at once.
pub fn any_entry_past(bits: u32, data: &[i64], entry_count: usize, limit: usize) -> bool {
    debug_assert!((1..=16).contains(&bits));
    if limit >= 1usize << bits {
        return false;
    }
    let per_long = entries_per_long(bits);
    let full = entry_count / per_long;
    let (lo_lanes, lo_carries, lo_addend) = even_lanes(bits, per_long, limit as u64);
    let (hi_lanes, hi_carries, hi_addend) = even_lanes(bits, per_long - 1, limit as u64);

    for &word in &data[..full] {
        let word = word as u64;
        let lo = (word & lo_lanes) + lo_addend;
        let hi = ((word >> bits) & hi_lanes) + hi_addend;
        if lo & lo_carries != 0 || hi & hi_carries != 0 {
            return true;
        }
    }

    let tail = entry_count % per_long;
    if tail == 0 {
        return false;
    }
    let mask = (1u64 << bits) - 1;
    let mut word = data[full] as u64;
    (0..tail).any(|_| {
        let lane = word & mask;
        word >>= bits;
        lane as usize >= limit
    })
}

/// Only the error path needs this, so it may be as slow as it likes.
pub fn first_entry_past(bits: u32, data: &[i64], entry_count: usize, limit: usize) -> Option<u16> {
    let per_long = entries_per_long(bits);
    let mask = (1u64 << bits) - 1;
    (0..entry_count)
        .map(|i| ((data[i / per_long] as u64 >> ((i % per_long) as u32 * bits)) & mask) as u16)
        .find(|&e| e as usize >= limit)
}

/// Packs one entry per element of `cells`, taking each entry's value from `id`.
/// The caller owns the width: `id` must never answer more than `bits` bits.
pub fn pack_from<T>(bits: u32, cells: &[T], mut id: impl FnMut(&T) -> u32) -> Box<[i64]> {
    debug_assert!((1..=32).contains(&bits));
    let per_long = entries_per_long(bits);
    cells
        .chunks(per_long)
        .map(|chunk| {
            chunk.iter().enumerate().fold(0i64, |word, (slot, cell)| {
                let value = id(cell) as u64;
                debug_assert!(value >> bits == 0);
                word | (value << (slot as u32 * bits)) as i64
            })
        })
        .collect()
}

/// Every stored entry must be a valid index into `table`; a caller that
/// validated at load already knows this.
pub fn remap_into<T: Copy>(
    bits: u32,
    data: &[i64],
    table: &[T],
    out: &mut [T],
) -> Result<(), DataLength> {
    check_len(bits, data, out.len())?;
    let per_long = entries_per_long(bits);
    let mask = (1u64 << bits) - 1;
    for (cells, &word) in out.chunks_mut(per_long).zip(data) {
        let mut word = word as u64;
        for cell in cells {
            *cell = table[(word & mask) as usize];
            word >>= bits;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
