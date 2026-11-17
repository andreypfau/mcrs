use core::cell::Cell;

use crate::access::{SectionProperties, SimulatedSections};
use crate::region::Region;
use crate::ticking_positions::local_index;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, Denied, Palette, PaletteBuffer, PalettedSectionMut,
    SectionBlocks, Unavailable, Volume,
};
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};
use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub(crate) struct Overlay {
    sections: FxHashMap<SectionPos, FxHashMap<u16, BlockStateId>>,
}

impl Overlay {
    pub(crate) fn clear(&mut self) {
        self.sections.clear();
    }
}

pub(crate) struct OverlaidVolume<'a, W: BlockVolume> {
    pub(crate) world: &'a W,
    pub(crate) region: Region,
    pub(crate) overlay: &'a mut Overlay,
    pub(crate) last: Cell<Option<(SectionPos, W::Section<'a>)>>,
}

impl<W: BlockVolume> Volume for OverlaidVolume<'_, W> {
    fn bounds(&self) -> BoundingBox {
        self.world.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        if self.region.holds(section) {
            self.world.access(section)
        } else {
            Access::Foreign
        }
    }
}

impl<'a, W: BlockVolume> BlockVolume for OverlaidVolume<'a, W> {
    type Section<'b>
        = OverlaidSection<'b, W::Section<'b>>
    where
        Self: 'b;

    fn section(&self, at: SectionPos) -> Result<Self::Section<'_>, Unavailable> {
        self.access(at).check_read()?;
        Ok(OverlaidSection {
            base: self.world.section(at)?,
            blocks: self.overlay.sections.get(&at),
        })
    }

    fn block(&self, at: BlockPos) -> Result<BlockStateId, Unavailable> {
        let section = SectionPos::from(at);
        let base = match self.last.take() {
            Some((held, base)) if held == section => base,
            _ if !self.region.holds(section) => return Err(Unavailable::Foreign),
            _ => {
                let world: &'a W = self.world;
                world.section(section)?
            }
        };
        let local = LocalPos::from(at);
        let state = self
            .overlay
            .sections
            .get(&section)
            .and_then(|written| written.get(&local_index(local)))
            .copied()
            .unwrap_or_else(|| base.get(local));
        self.last.set(Some((section, base)));
        Ok(state)
    }
}

impl<W: BlockVolume> BlockVolumeMut for OverlaidVolume<'_, W> {
    type SectionMut<'b>
        = PalettedSectionMut<'b>
    where
        Self: 'b;

    fn section_mut(&mut self, _at: SectionPos) -> Result<Self::SectionMut<'_>, Denied> {
        Err(Denied::ReadOnly)
    }

    fn set_block(&mut self, at: BlockPos, state: BlockStateId) -> Result<BlockStateId, Denied> {
        let section = SectionPos::from(at);
        self.access(section).check_write()?;
        let old = self.block(at)?;
        self.overlay
            .sections
            .entry(section)
            .or_default()
            .insert(local_index(LocalPos::from(at)), state);
        Ok(old)
    }
}

impl<W: BlockVolume + SimulatedSections> SimulatedSections for OverlaidVolume<'_, W> {
    fn properties(&self, section: SectionPos) -> SectionProperties {
        self.world.properties(section)
    }
}

pub(crate) struct OverlaidSection<'a, S> {
    base: S,
    blocks: Option<&'a FxHashMap<u16, BlockStateId>>,
}

impl<S: SectionBlocks> SectionBlocks for OverlaidSection<'_, S> {
    fn get(&self, at: LocalPos) -> BlockStateId {
        match self.blocks.and_then(|blocks| blocks.get(&local_index(at))) {
            Some(&state) => state,
            None => self.base.get(at),
        }
    }

    fn palette<'s>(&'s self, buffer: &'s mut PaletteBuffer) -> Palette<'s> {
        match self.blocks {
            None => self.base.palette(buffer),
            Some(_) => buffer.collect(self),
        }
    }
}
