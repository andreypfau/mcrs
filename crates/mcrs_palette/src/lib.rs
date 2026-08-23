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

/// Unpacks `data` into one entry per element of `out`, low entry first, with the
/// spare high bits of each word discarded. `bits` must be in `1..=16`.
pub fn unpack_into(bits: u32, data: &[i64], out: &mut [u16]) -> Result<(), DataLength> {
    debug_assert!((1..=16).contains(&bits));
    let per_long = entries_per_long(bits);
    let expected = out.len().div_ceil(per_long);
    if data.len() != expected {
        return Err(DataLength {
            found: data.len(),
            expected,
        });
    }
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

/// Unpacks and translates in one pass through a flat `2^bits` table, the shape
/// Lithium uses instead of a hash probe per cell.
pub fn remap_into(
    bits: u32,
    data: &[i64],
    table: &[u16],
    out: &mut [u16],
) -> Result<(), DataLength> {
    debug_assert_eq!(table.len(), 1usize << bits);
    unpack_into(bits, data, out)?;
    for cell in out {
        *cell = table[*cell as usize];
    }
    Ok(())
}

#[cfg(test)]
mod tests;
