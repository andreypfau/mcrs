use std::collections::BTreeMap;
use std::ops::ControlFlow;

use mcrs_minecraft_chunk::PalettedContainer;
use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::{BlockPos, SectionPos};

use crate::access::{Access, Denied, Unavailable, Volume};
use crate::access::{section_inside, section_meets};
use crate::block::{BlockVolume, BlockVolumeMut};
use crate::bounds::SectionBox;
use crate::paletted::{Container, PalettedSection, PalettedSectionMut};
use crate::visit::{Missing, SectionVolume, VisitOptions, VisitOrder, walk};

/// Sections keyed so that ascending order is `y`, then `z`, then `x`.
type Key = (i32, i32, i32);

const fn key(at: SectionPos) -> Key {
    (at.0.y, at.0.z, at.0.x)
}

fn position((y, z, x): Key) -> SectionPos {
    SectionPos::new(x, y, z)
}

/// Paletted sections where they are present and nothing elsewhere: a test world, a
/// structure template, sections read from a save. An absent section inside the bounds is
/// [`Access::Unloaded`]; a present one is writable. Only a section wholly inside the
/// bounds can be present, so the cells of a section the bounds cut stay unloaded.
#[derive(Clone, Debug)]
pub struct SparseVolume {
    bounds: BoundingBox,
    sections: BTreeMap<Key, Container>,
}

impl SparseVolume {
    pub fn new(bounds: BoundingBox) -> Self {
        Self {
            bounds,
            sections: BTreeMap::new(),
        }
    }

    pub fn unbounded() -> Self {
        Self::new(BoundingBox {
            min: BlockPos::new(i32::MIN, i32::MIN, i32::MIN),
            max: BlockPos::new(i32::MAX, i32::MAX, i32::MAX),
        })
    }

    /// Makes the section present with every cell `state`, replacing what it held. Refused,
    /// answering `false`, for a section not wholly inside the bounds.
    pub fn insert(&mut self, at: SectionPos, state: BlockStateId) -> bool {
        if !section_inside(self.bounds, at) {
            return false;
        }
        self.sections
            .insert(key(at), PalettedContainer::Homogeneous(state));
        true
    }

    /// Answers whether the section was present.
    pub fn remove(&mut self, at: SectionPos) -> bool {
        self.sections.remove(&key(at)).is_some()
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

impl Volume for SparseVolume {
    fn bounds(&self) -> BoundingBox {
        self.bounds
    }

    fn access(&self, section: SectionPos) -> Access {
        if !section_meets(self.bounds, section) {
            Access::Outside
        } else if self.sections.contains_key(&key(section)) {
            Access::Write
        } else {
            Access::Unloaded
        }
    }
}

impl BlockVolume for SparseVolume {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        match self.sections.get(&key(at)) {
            Some(section) => Ok(PalettedSection(section)),
            None if section_meets(self.bounds, at) => Err(Unavailable::Unloaded),
            None => Err(Unavailable::Outside),
        }
    }
}

impl BlockVolumeMut for SparseVolume {
    type SectionMut<'a> = PalettedSectionMut<'a>;

    fn section_mut(&mut self, at: SectionPos) -> Result<PalettedSectionMut<'_>, Denied> {
        let why = if section_meets(self.bounds, at) {
            Denied::Unloaded
        } else {
            Denied::Outside
        };
        self.sections
            .get_mut(&key(at))
            .map(PalettedSectionMut)
            .ok_or(why)
    }
}

impl SectionVolume for SparseVolume {
    fn visit_sections<B, F>(
        &self,
        area: SectionBox,
        options: VisitOptions,
        mut f: F,
    ) -> ControlFlow<B>
    where
        F: FnMut(SectionPos, Result<PalettedSection<'_>, Unavailable>) -> ControlFlow<B>,
    {
        // chisle: column-major order and reported holes walk the whole area; a second
        // index keyed (x, z, y) would let column-major walk only what is held.
        if options.missing == Missing::Report || options.order == VisitOrder::ColumnMajor {
            return walk(self, area, options, f);
        }
        if area.is_empty() {
            return ControlFlow::Continue(());
        }
        self.sections
            .range(key(area.min)..=key(area.max))
            .map(|(held, section)| (position(*held), section))
            .filter(|(at, _)| area.contains(*at))
            .try_for_each(|(at, section)| f(at, Ok(PalettedSection(section))))
    }
}
