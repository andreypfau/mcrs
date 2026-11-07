//! The sections of a dimension world as the block simulation sees them.

use bevy_ecs::prelude::{Query, With};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::BlockTicking;
use mcrs_minecraft_engine_world::world::storage::view::DimensionViewMut;
use mcrs_minecraft_simulation::{SectionProperties, SimulatedSections, TickingSectionMut};
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, ColumnLayout, Denied, PalettedSection, PalettedSectionMut,
    Unavailable, Volume,
};
use mcrs_voxel::{ColumnPos, SectionPos};

/// A loaded section of a column that the cell owns is writable. A loaded section of the halo is
/// readable only: the kernels send a write, a tick, an event or an update into it to its owner.
pub struct CellVolume<'v, 'a, 'w, 's> {
    pub(crate) view: DimensionViewMut<'a, 'w, 's>,
    pub(crate) owned: Option<&'v OwnedColumns>,
    pub(crate) ticking: &'v TickingSections<'v, 'v>,
}

/// The sections that run scheduled ticks and deliver block events.
pub(crate) type TickingSections<'w, 's> = Query<'w, 's, (), With<BlockTicking>>;

impl CellVolume<'_, '_, '_, '_> {
    fn owns(&self, section: SectionPos) -> bool {
        owned_columns::owns(self.owned, ColumnPos::from(section))
    }
}

impl Volume for CellVolume<'_, '_, '_, '_> {
    fn bounds(&self) -> BoundingBox {
        self.view.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        match self.view.access(section) {
            Access::Write if !self.owns(section) => Access::Read,
            access => access,
        }
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        self.view.column_layout()
    }
}

impl SimulatedSections for CellVolume<'_, '_, '_, '_> {
    fn properties(&self, section: SectionPos) -> SectionProperties {
        if self.access(section) != Access::Write {
            return SectionProperties::empty();
        }
        let loaded = SectionProperties::ACCEPTS_TICKS | SectionProperties::BROADCASTS_CHANGES;
        let ticking = self
            .view
            .section_entity(section)
            .is_some_and(|entity| self.ticking.contains(entity));
        if ticking {
            loaded
                | SectionProperties::RUNS_SCHEDULED_TICKS
                | SectionProperties::DELIVERS_BLOCK_EVENTS
        } else {
            loaded
        }
    }

    fn random_ticking(&self, section: SectionPos) -> &[u16] {
        self.view
            .random_ticks()
            .map_or(&[], |index| index.ticking(section))
    }
}

impl BlockVolume for CellVolume<'_, '_, '_, '_> {
    type Section<'b>
        = PalettedSection<'b>
    where
        Self: 'b;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        self.view.section(at)
    }
}

impl BlockVolumeMut for CellVolume<'_, '_, '_, '_> {
    type SectionMut<'b>
        = TickingSectionMut<'b, PalettedSectionMut<'b>>
    where
        Self: 'b;

    fn section_mut(&mut self, at: SectionPos) -> Result<Self::SectionMut<'_>, Denied> {
        if !self.owns(at) {
            return Err(Denied::ReadOnly);
        }
        self.view.section_mut(at)
    }
}
