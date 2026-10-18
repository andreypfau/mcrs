use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::{BlockPos, LocalPos, SectionPos};

use crate::access::{Denied, Unavailable, Volume};

/// A volume whose block states can be read, a section at a time.
pub trait BlockVolume: Volume {
    type Section<'a>: SectionBlocks
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<Self::Section<'_>, Unavailable>;

    #[inline]
    fn block(&self, at: BlockPos) -> Result<BlockStateId, Unavailable> {
        if !self.contains(at) {
            return Err(Unavailable::Outside);
        }
        self.section(SectionPos::from(at))
            .map(|section| section.get(LocalPos::from(at)))
    }
}

/// A volume whose block states can be written in place. A write is storage only: no
/// neighbour updates, light or heightmaps.
pub trait BlockVolumeMut: BlockVolume {
    type SectionMut<'a>: SectionBlocksMut
    where
        Self: 'a;

    fn section_mut(&mut self, at: SectionPos) -> Result<Self::SectionMut<'_>, Denied>;

    /// Answers the state the write replaced.
    #[inline]
    fn set_block(&mut self, at: BlockPos, state: BlockStateId) -> Result<BlockStateId, Denied> {
        if !self.contains(at) {
            return Err(Denied::Outside);
        }
        self.section_mut(SectionPos::from(at))
            .map(|mut section| section.set(LocalPos::from(at), state))
    }
}

/// One 16³ section of block states.
pub trait SectionBlocks {
    fn get(&self, at: LocalPos) -> BlockStateId;

    fn uniform(&self) -> Option<BlockStateId> {
        let first = self.get(LocalPos::from_index(0));
        LocalPos::all()
            .all(|at| self.get(at) == first)
            .then_some(first)
    }

    /// The distinct states with their counts. A handle over paletted storage borrows its
    /// own palette and leaves `buffer` alone; any other handle builds it in `buffer`.
    fn palette<'s>(&'s self, buffer: &'s mut PaletteBuffer) -> Palette<'s> {
        buffer.collect(self)
    }

    /// Every cell in `y`, `z`, `x` order, `x` fastest.
    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(LocalPos, BlockStateId),
    {
        for at in LocalPos::all() {
            f(at, self.get(at));
        }
    }

    /// Cells indexed by [`LocalPos::index`].
    fn copy_to(&self, out: &mut [BlockStateId; SectionPos::VOLUME]) {
        self.for_each(|at, state| {
            if let Some(cell) = out.get_mut(at.index()) {
                *cell = state;
            }
        });
    }
}

pub trait SectionBlocksMut: SectionBlocks {
    /// Answers the state the write replaced.
    fn set(&mut self, at: LocalPos, state: BlockStateId) -> BlockStateId;

    fn fill(&mut self, state: BlockStateId) {
        for at in LocalPos::all() {
            self.set(at, state);
        }
    }

    /// Fills `min..=max` on every axis; nothing when `min` exceeds `max` on one.
    fn fill_box(&mut self, min: LocalPos, max: LocalPos, state: BlockStateId) {
        for y in min.y()..=max.y() {
            for z in min.z()..=max.z() {
                for x in min.x()..=max.x() {
                    self.set(LocalPos::new(x, y, z), state);
                }
            }
        }
    }
}

/// The distinct states of a section, each with the number of cells holding it.
#[derive(Clone, Copy, Debug)]
pub struct Palette<'a>(Entries<'a>);

#[derive(Clone, Copy, Debug)]
enum Entries<'a> {
    Uniform(BlockStateId),
    Listed {
        values: &'a [BlockStateId],
        counts: &'a [u16],
    },
}

impl<'a> Palette<'a> {
    pub(crate) const fn uniform(state: BlockStateId) -> Self {
        Self(Entries::Uniform(state))
    }

    pub(crate) const fn listed(values: &'a [BlockStateId], counts: &'a [u16]) -> Self {
        Self(Entries::Listed { values, counts })
    }

    pub fn len(&self) -> usize {
        match self.0 {
            Entries::Uniform(_) => 1,
            Entries::Listed { values, .. } => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (BlockStateId, u16)> + 'a {
        let (uniform, values, counts) = match self.0 {
            Entries::Uniform(state) => (Some(state), [].as_slice(), [].as_slice()),
            Entries::Listed { values, counts } => (None, values, counts),
        };
        uniform
            .map(|state| (state, SectionPos::VOLUME as u16))
            .into_iter()
            .chain(
                values
                    .iter()
                    .zip(counts)
                    .map(|(value, count)| (*value, *count)),
            )
    }

    pub fn any<F>(&self, mut f: F) -> bool
    where
        F: FnMut(BlockStateId) -> bool,
    {
        self.iter().any(|(state, _)| f(state))
    }

    /// The number of cells whose state passes `f`.
    pub fn count<F>(&self, mut f: F) -> u16
    where
        F: FnMut(BlockStateId) -> bool,
    {
        self.iter()
            .filter(|(state, _)| f(*state))
            .fold(0u16, |sum, (_, count)| sum.saturating_add(count))
    }
}

/// Scratch for the palette of a section that does not keep one. Reused across calls, it
/// stops allocating once it has held the largest palette asked of it.
#[derive(Clone, Debug, Default)]
pub struct PaletteBuffer {
    values: Vec<BlockStateId>,
    counts: Vec<u16>,
}

impl PaletteBuffer {
    fn collect<S: SectionBlocks + ?Sized>(&mut self, section: &S) -> Palette<'_> {
        self.values.clear();
        self.counts.clear();
        let mut last: Option<(BlockStateId, usize)> = None;
        section.for_each(|_, state| {
            let slot = match last {
                Some((previous, slot)) if previous == state => slot,
                _ => match self.values.binary_search(&state) {
                    Ok(slot) => slot,
                    Err(slot) => {
                        self.values.insert(slot, state);
                        self.counts.insert(slot, 0);
                        slot
                    }
                },
            };
            last = Some((state, slot));
            if let Some(count) = self.counts.get_mut(slot) {
                *count = count.saturating_add(1);
            }
        });
        Palette::listed(&self.values, &self.counts)
    }
}
