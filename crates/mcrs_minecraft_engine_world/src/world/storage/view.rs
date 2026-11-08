use bevy_ecs::prelude::{Entity, Query};
use bevy_ecs::system::{ResMut, SystemParam};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_simulation::TickingSectionMut;
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, ColumnLayout, Denied, PalettedSection, PalettedSectionMut,
    SectionVolume, Unavailable, Volume,
};
use mcrs_voxel::{BlockPos, SectionPos};

use crate::world::dimension::DimensionTypeConfig;
use crate::world::storage::random_ticks::RandomTickIndex;
use crate::world::storage::section::SectionIndex;

/// Shared block access to the dimensions of a world.
#[derive(SystemParam)]
pub struct WorldView<'w, 's> {
    dimensions: Query<'w, 's, (&'static SectionIndex, &'static DimensionTypeConfig)>,
}

/// Block access to the dimensions of a world that may write their sections.
#[derive(SystemParam)]
pub struct WorldViewMut<'w, 's> {
    dimensions: Query<'w, 's, (&'static mut SectionIndex, &'static DimensionTypeConfig)>,
    random_ticks: Option<ResMut<'w, RandomTickIndex>>,
}

impl WorldView<'_, '_> {
    /// `None` for an entity that is not a dimension.
    pub fn get(&self, dimension: Entity) -> Option<DimensionView<'_>> {
        let (index, config) = self.dimensions.get(dimension).ok()?;
        Some(DimensionView {
            index,
            layout: layout(config),
        })
    }
}

impl WorldViewMut<'_, '_> {
    /// `None` for an entity that is not a dimension.
    pub fn get_mut(&mut self, dimension: Entity) -> Option<DimensionViewMut<'_>> {
        let (index, config) = self.dimensions.get_mut(dimension).ok()?;
        Some(DimensionViewMut {
            layout: layout(config),
            index: index.into_inner(),
            random_ticks: self.random_ticks.as_deref_mut(),
        })
    }
}

fn layout(config: &DimensionTypeConfig) -> ColumnLayout {
    ColumnLayout {
        min_section_y: config.min_y.div_euclid(SectionPos::SIZE as i32),
        sections: config.section_count,
    }
}

/// The loaded sections of one dimension, read-only. A section is readable once its blocks
/// landed; one still generating is [`Access::Unloaded`].
pub struct DimensionView<'a> {
    index: &'a SectionIndex,
    layout: ColumnLayout,
}

/// The loaded sections of one dimension, writable. A write diverges the section from
/// every snapshot taken of it and is storage only: nothing hears of it, except the
/// [`RandomTickIndex`] when the world holds one.
pub struct DimensionViewMut<'a> {
    index: &'a mut SectionIndex,
    layout: ColumnLayout,
    random_ticks: Option<&'a mut RandomTickIndex>,
}

impl DimensionViewMut<'_> {
    pub fn section_entity(&self, at: SectionPos) -> Option<Entity> {
        self.index.get(at)
    }

    pub fn random_ticks_mut(&mut self) -> Option<&mut RandomTickIndex> {
        self.random_ticks.as_deref_mut()
    }
}

fn bounds(layout: ColumnLayout) -> BoundingBox {
    let top = layout
        .min_y()
        .saturating_add_unsigned(layout.height())
        .saturating_sub(1);
    BoundingBox {
        min: BlockPos::new(i32::MIN, layout.min_y(), i32::MIN),
        max: BlockPos::new(i32::MAX, top, i32::MAX),
    }
}

fn section(
    index: &SectionIndex,
    layout: ColumnLayout,
    at: SectionPos,
) -> Result<PalettedSection<'_>, Unavailable> {
    if !layout.contains_section_y(at.y) {
        return Err(Unavailable::Outside);
    }
    let blocks = index.blocks(at).ok_or(Unavailable::Unloaded)?;
    Ok(PalettedSection(&blocks.0.0))
}

fn access(index: &SectionIndex, layout: ColumnLayout, at: SectionPos, granted: Access) -> Access {
    match section(index, layout, at) {
        Ok(_) => granted,
        Err(Unavailable::Outside) => Access::Outside,
        Err(_) => Access::Unloaded,
    }
}

impl Volume for DimensionView<'_> {
    fn bounds(&self) -> BoundingBox {
        bounds(self.layout)
    }

    fn access(&self, at: SectionPos) -> Access {
        access(self.index, self.layout, at, Access::Read)
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        Some(self.layout)
    }
}

impl BlockVolume for DimensionView<'_> {
    type Section<'a>
        = PalettedSection<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        section(self.index, self.layout, at)
    }
}

impl SectionVolume for DimensionView<'_> {}

impl Volume for DimensionViewMut<'_> {
    fn bounds(&self) -> BoundingBox {
        bounds(self.layout)
    }

    fn access(&self, at: SectionPos) -> Access {
        access(self.index, self.layout, at, Access::Write)
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        Some(self.layout)
    }
}

impl BlockVolume for DimensionViewMut<'_> {
    type Section<'a>
        = PalettedSection<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        section(self.index, self.layout, at)
    }
}

impl BlockVolumeMut for DimensionViewMut<'_> {
    type SectionMut<'a>
        = TickingSectionMut<'a, PalettedSectionMut<'a>>
    where
        Self: 'a;

    fn section_mut(&mut self, at: SectionPos) -> Result<Self::SectionMut<'_>, Denied> {
        if !self.layout.contains_section_y(at.y) {
            return Err(Denied::Outside);
        }
        self.index
            .section_mut(at, self.random_ticks.as_deref_mut())
            .ok_or(Denied::Unloaded)
    }
}

impl SectionVolume for DimensionViewMut<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::air_section;
    use bevy_ecs::system::SystemState;
    use bevy_ecs::world::World;
    use mcrs_voxel::BlockStateId;

    #[test]
    fn a_section_answers_by_what_the_dimension_holds_for_it() {
        let mut world = World::new();
        let (loaded, generating) = (SectionPos::new(0, 0, 0), SectionPos::new(1, 0, 0));
        let mut index = SectionIndex::new();
        index.insert(loaded, world.spawn_empty().id());
        index.insert(generating, world.spawn_empty().id());
        let entity = index.get(loaded).expect("an indexed section");
        index.land(loaded, entity, air_section(), None, false);
        let dimension = world.spawn((index, DimensionTypeConfig::new(0, 32))).id();

        let mut state: SystemState<WorldViewMut> = SystemState::new(&mut world);
        let mut blocks = state.get_mut(&mut world).expect("valid params");
        let mut view = blocks.get_mut(dimension).expect("a dimension");

        assert_eq!(view.access(loaded), Access::Write);
        assert_eq!(view.access(generating), Access::Unloaded);
        assert_eq!(view.access(SectionPos::new(2, 0, 0)), Access::Unloaded);
        assert_eq!(view.access(SectionPos::new(0, 2, 0)), Access::Outside);
        assert_eq!(view.access(SectionPos::new(0, -1, 0)), Access::Outside);
        assert_eq!(
            view.set_block(BlockPos::new(20, 3, 0), BlockStateId::AIR),
            Err(Denied::Unloaded)
        );
        assert_eq!(
            view.block(BlockPos::new(3, 32, 0)),
            Err(Unavailable::Outside)
        );

        let stone = BlockStateId::checked(1, 2).expect("a state past air");
        assert_eq!(
            view.set_block(BlockPos::new(3, 4, 5), stone),
            Ok(BlockStateId::AIR)
        );
        assert_eq!(view.block(BlockPos::new(3, 4, 5)), Ok(stone));
        assert_eq!(view.block(BlockPos::new(3, 4, 6)), Ok(BlockStateId::AIR));
    }
}
