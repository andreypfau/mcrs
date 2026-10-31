use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::{BlockPos, SectionPos};

use crate::bounds::ColumnLayout;

/// What the holder of a volume may do with one section now, weakest first.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Access {
    Outside,
    Unloaded,
    /// Loaded, but leased to another writer: not readable now.
    Foreign,
    /// Readable and stable: nobody writes it while this holder lives.
    Read,
    Write,
}

impl Access {
    pub const fn meet(self, other: Self) -> Self {
        if (self as u8) <= (other as u8) {
            self
        } else {
            other
        }
    }

    pub const fn check_read(self) -> Result<(), Unavailable> {
        match self {
            Access::Read | Access::Write => Ok(()),
            Access::Foreign => Err(Unavailable::Foreign),
            Access::Unloaded => Err(Unavailable::Unloaded),
            Access::Outside => Err(Unavailable::Outside),
        }
    }

    pub const fn check_write(self) -> Result<(), Denied> {
        match self {
            Access::Write => Ok(()),
            Access::Read => Err(Denied::ReadOnly),
            Access::Foreign => Err(Denied::Foreign),
            Access::Unloaded => Err(Denied::Unloaded),
            Access::Outside => Err(Denied::Outside),
        }
    }
}

/// Why a read did not happen.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, thiserror::Error)]
pub enum Unavailable {
    #[error("outside the volume")]
    Outside,
    #[error("section not loaded")]
    Unloaded,
    #[error("section held by another writer")]
    Foreign,
}

/// Why a write did not happen.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, thiserror::Error)]
pub enum Denied {
    #[error("outside the volume")]
    Outside,
    #[error("section not loaded")]
    Unloaded,
    #[error("section held by another writer")]
    Foreign,
    #[error("section is read-only here")]
    ReadOnly,
}

impl From<Unavailable> for Denied {
    fn from(why: Unavailable) -> Self {
        match why {
            Unavailable::Outside => Denied::Outside,
            Unavailable::Unloaded => Denied::Unloaded,
            Unavailable::Foreign => Denied::Foreign,
        }
    }
}

/// A region of the world with fixed bounds, answering per section what its holder may do.
/// Nothing is ever loaded on access.
pub trait Volume {
    /// Inclusive, and fixed for the volume's lifetime; an unbounded axis spans the `i32` range.
    fn bounds(&self) -> BoundingBox;

    fn access(&self, section: SectionPos) -> Access;

    /// The vertical extent, for a volume whose bounds are whole columns.
    fn column_layout(&self) -> Option<ColumnLayout> {
        None
    }

    #[inline]
    fn contains(&self, pos: BlockPos) -> bool {
        within(self.bounds(), pos)
    }
}

pub(crate) fn within(bounds: BoundingBox, pos: BlockPos) -> bool {
    (bounds.min.cmple(*pos) & pos.cmple(*bounds.max)).all()
}

pub(crate) fn section_meets(bounds: BoundingBox, at: SectionPos) -> bool {
    section_axes(bounds, at, |first, last, lo, hi| first <= hi && last >= lo)
}

pub(crate) fn section_inside(bounds: BoundingBox, at: SectionPos) -> bool {
    section_axes(bounds, at, |first, last, lo, hi| first >= lo && last <= hi)
}

fn section_axes<T>(bounds: BoundingBox, at: SectionPos, test: T) -> bool
where
    T: Fn(i64, i64, i64, i64) -> bool,
{
    let axis = |section: i32, lo: i32, hi: i32| {
        let first = i64::from(section).wrapping_mul(SectionPos::SIZE as i64);
        let last = first.wrapping_add(SectionPos::MASK as i64);
        lo <= hi && test(first, last, i64::from(lo), i64::from(hi))
    };
    axis(at.x, bounds.min.x, bounds.max.x)
        && axis(at.y, bounds.min.y, bounds.max.y)
        && axis(at.z, bounds.min.z, bounds.max.z)
}
