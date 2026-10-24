use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::{BlockPos, BlockStateId, BoundingBox, SectionPos};

use crate::access::{Access, Unavailable, Volume};
use crate::block::BlockVolume;
use crate::bounds::{ColumnLayout, SectionBox};
use crate::paletted::PalettedSection;
use crate::visit::{SectionVolume, VisitOrder};

/// One section's block states, shared between a store and every snapshot taken of it.
pub type SharedSection = Arc<VoxelPalette<BlockStateId, { SectionPos::SIZE }>>;

/// The sections of a box as they were when it was taken. Each held section shares its
/// storage with the store it came from, so a snapshot costs a reference count per
/// section and the store's later writes leave it as it was. Every section is
/// [`Access::Read`] or keeps the reason the store could not give it.
#[derive(Clone, Debug)]
pub struct Snapshot {
    area: SectionBox,
    layout: Option<ColumnLayout>,
    sections: Box<[Result<SharedSection, Unavailable>]>,
}

impl Snapshot {
    /// Asks `section` for every section of `area`, `y` then `z` then `x`, `x` fastest.
    pub fn take<F>(area: SectionBox, layout: Option<ColumnLayout>, section: F) -> Self
    where
        F: FnMut(SectionPos) -> Result<SharedSection, Unavailable>,
    {
        Self {
            area,
            layout,
            sections: area.iter(VisitOrder::SectionMajor).map(section).collect(),
        }
    }

    pub fn shared(&self, at: SectionPos) -> Result<&SharedSection, Unavailable> {
        match self
            .sections
            .get(self.index(at).ok_or(Unavailable::Outside)?)
        {
            Some(Ok(section)) => Ok(section),
            Some(Err(why)) => Err(*why),
            None => Err(Unavailable::Outside),
        }
    }

    fn index(&self, at: SectionPos) -> Option<usize> {
        if !self.area.contains(at) {
            return None;
        }
        let offset =
            |at: i32, min: i32| usize::try_from(i64::from(at).wrapping_sub(i64::from(min))).ok();
        let width = offset(self.area.max.x, self.area.min.x)?.checked_add(1)?;
        let depth = offset(self.area.max.z, self.area.min.z)?.checked_add(1)?;
        let (x, y, z) = (
            offset(at.x, self.area.min.x)?,
            offset(at.y, self.area.min.y)?,
            offset(at.z, self.area.min.z)?,
        );
        y.checked_mul(depth)?
            .checked_add(z)?
            .checked_mul(width)?
            .checked_add(x)
    }
}

impl Volume for Snapshot {
    fn bounds(&self) -> BoundingBox {
        let edge = |section: i32, last: bool| {
            let first = section.saturating_mul(SectionPos::SIZE as i32);
            if last {
                first.saturating_add(SectionPos::MASK as i32)
            } else {
                first
            }
        };
        let (min, max) = (self.area.min, self.area.max);
        BoundingBox {
            min: BlockPos::new(edge(min.x, false), edge(min.y, false), edge(min.z, false)),
            max: BlockPos::new(edge(max.x, true), edge(max.y, true), edge(max.z, true)),
        }
    }

    fn access(&self, section: SectionPos) -> Access {
        match self.shared(section) {
            Ok(_) => Access::Read,
            Err(Unavailable::Outside) => Access::Outside,
            Err(Unavailable::Unloaded) => Access::Unloaded,
            Err(Unavailable::Foreign) => Access::Foreign,
        }
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        self.layout
    }
}

impl BlockVolume for Snapshot {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        self.shared(at).map(|section| PalettedSection(&section.0))
    }
}

impl SectionVolume for Snapshot {}
