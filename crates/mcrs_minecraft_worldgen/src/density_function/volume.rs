use super::{BufferPool, NO_SLOT};
use bevy_math::IVec3;
use std::cell::RefCell;

/// A strided box of block positions: `size` samples per axis, starting at
/// `min_block`, spaced `step_block` apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    size: IVec3,
    min_block: IVec3,
    step_block: IVec3,
}

#[allow(clippy::len_without_is_empty)]
impl Volume {
    pub fn new(size: IVec3, min_block: IVec3, step_block: IVec3) -> Self {
        assert!(
            size.x > 0 && size.y > 0 && size.z > 0,
            "size must be positive, was: {}x{}x{}",
            size.x,
            size.y,
            size.z
        );
        assert!(
            step_block.x > 0 && step_block.y > 0 && step_block.z > 0,
            "step must be positive, was: {}; {}; {}",
            step_block.x,
            step_block.y,
            step_block.z
        );
        Self {
            size,
            min_block,
            step_block,
        }
    }

    pub fn dense(size: IVec3, min_block: IVec3) -> Self {
        Self::new(size, min_block, IVec3::ONE)
    }

    pub fn point(pos: IVec3) -> Self {
        Self::new(IVec3::ONE, pos, IVec3::ONE)
    }

    #[inline]
    pub fn size(&self) -> IVec3 {
        self.size
    }

    #[inline]
    pub fn min_block(&self) -> IVec3 {
        self.min_block
    }

    #[inline]
    pub fn step_block(&self) -> IVec3 {
        self.step_block
    }

    #[inline]
    pub fn index_unchecked(&self, x: i32, y: i32, z: i32) -> usize {
        (y + (x + z * self.size.x) * self.size.y) as usize
    }

    #[inline]
    pub fn block_x(&self, x: i32) -> i32 {
        self.min_block.x + x * self.step_block.x
    }

    #[inline]
    pub fn block_y(&self, y: i32) -> i32 {
        self.min_block.y + y * self.step_block.y
    }

    #[inline]
    pub fn block_z(&self, z: i32) -> i32 {
        self.min_block.z + z * self.step_block.z
    }

    #[inline]
    pub fn max_block(&self) -> IVec3 {
        self.min_block + self.size * self.step_block - IVec3::ONE
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.size.x * self.size.y * self.size.z) as usize
    }

    pub(super) fn positions_into(&self, out: &mut [IVec3]) {
        debug_assert_eq!(out.len(), self.len());
        let mut i = 0;
        for z in 0..self.size.z {
            let bz = self.block_z(z);
            for x in 0..self.size.x {
                let bx = self.block_x(x);
                for y in 0..self.size.y {
                    out[i] = IVec3::new(bx, self.block_y(y), bz);
                    i += 1;
                }
            }
        }
    }

    /// `self` flattened onto a single Y layer: the key under which a value that
    /// holds for a whole column is cached.
    pub(super) fn column(&self) -> Volume {
        Volume {
            size: self.size.with_y(1),
            min_block: self.min_block.with_y(0),
            step_block: self.step_block.with_y(1),
        }
    }
}

/// Buffers a caller keeps across fills.
///
/// Reusing one across the cells of a chunk column is what makes the column-only
/// half of the graph cost once rather than once per cell.
#[derive(Default)]
pub struct FillScratch {
    pub(super) pool: BufferPool,
    pub(super) column: RefCell<ColumnCache>,
}

impl FillScratch {
    pub fn new() -> Self {
        Self::default()
    }
}

/// How many distinct columns the cache holds at once.
///
/// A chunk column asks about two: the block volume of the cell being filled,
/// and the cell lattice its interpolations resample on. The corner pass over a
/// whole chunk adds a third.
const COLUMNS_HELD: usize = 4;

/// The rows of every entry whose value holds for a whole column, for the last
/// few columns anyone asked about.
///
/// Entries accumulate within a column: a fill that needs one the cache has not
/// seen computes it and leaves it behind, so the next fill over that column —
/// the cell above, or a different root — pays only for what is new.
#[derive(Default)]
pub(super) struct ColumnCache {
    held: Vec<Column>,
    clock: u64,
}

#[derive(Default)]
pub(super) struct Column {
    volume: Option<Volume>,
    pub(super) rows: Vec<f32>,
    pub(super) slots: Vec<u32>,
    pub(super) positions: Vec<IVec3>,
    claimed: usize,
    used: u64,
}

impl ColumnCache {
    /// The entry holding `volume`, evicting the column left untouched longest
    /// if there is no room for it.
    pub(super) fn column(&mut self, volume: &Volume, entries: usize) -> &mut Column {
        self.clock += 1;
        let hit = self.held.iter().position(|c| c.volume.as_ref() == Some(volume));
        let index = match hit {
            Some(index) => index,
            None => {
                let index = if self.held.len() < COLUMNS_HELD {
                    self.held.push(Column::default());
                    self.held.len() - 1
                } else {
                    let mut oldest = 0;
                    for (i, column) in self.held.iter().enumerate() {
                        if column.used < self.held[oldest].used {
                            oldest = i;
                        }
                    }
                    oldest
                };
                self.held[index].reset(volume, entries);
                index
            }
        };
        self.held[index].used = self.clock;
        &mut self.held[index]
    }
}

impl Column {
    fn reset(&mut self, volume: &Volume, entries: usize) {
        self.volume = Some(*volume);
        self.slots.clear();
        self.slots.resize(entries, NO_SLOT);
        self.claimed = 0;
        self.positions.resize(volume.len(), IVec3::ZERO);
        volume.positions_into(&mut self.positions[..volume.len()]);
    }

    /// Give `entry` a row if it has none, reporting whether it still has to be
    /// filled.
    pub(super) fn claim(&mut self, entry: usize, columns: usize) -> bool {
        if self.slots[entry] != NO_SLOT {
            return false;
        }
        self.slots[entry] = self.claimed as u32;
        self.claimed += 1;
        self.rows.resize(self.claimed * columns, 0.0);
        true
    }

    #[inline]
    pub(super) fn row(&self, entry: usize, columns: usize) -> &[f32] {
        let base = self.slots[entry] as usize * columns;
        &self.rows[base..base + columns]
    }
}
