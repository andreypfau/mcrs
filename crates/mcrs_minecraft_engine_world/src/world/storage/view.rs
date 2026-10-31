use bevy_ecs::prelude::{Entity, Query};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, ColumnLayout, Denied, PalettedSection, PalettedSectionMut,
    SectionVolume, Unavailable, Volume,
};
use mcrs_voxel::{BlockPos, SectionPos};

use crate::palette::ChunkBlocks;
use crate::world::dimension::DimensionTypeConfig;
use crate::world::storage::section::SectionIndex;

type Dimensions<'w, 's> = Query<'w, 's, (&'static SectionIndex, &'static DimensionTypeConfig)>;

/// Shared block access to the dimensions of a world.
#[derive(SystemParam)]
pub struct WorldView<'w, 's> {
    dimensions: Dimensions<'w, 's>,
    sections: Query<'w, 's, &'static ChunkBlocks>,
}

/// Block access to the dimensions of a world that may write their sections.
#[derive(SystemParam)]
pub struct WorldViewMut<'w, 's> {
    dimensions: Dimensions<'w, 's>,
    sections: Query<'w, 's, &'static mut ChunkBlocks>,
}

impl<'w, 's> WorldView<'w, 's> {
    /// `None` for an entity that is not a dimension.
    pub fn get(&self, dimension: Entity) -> Option<DimensionView<'_, 'w, 's>> {
        Some(DimensionView {
            sections: Sections::of(&self.dimensions, dimension)?,
            blocks: &self.sections,
        })
    }
}

impl<'w, 's> WorldViewMut<'w, 's> {
    /// `None` for an entity that is not a dimension.
    pub fn get_mut(&mut self, dimension: Entity) -> Option<DimensionViewMut<'_, 'w, 's>> {
        Some(DimensionViewMut {
            sections: Sections::of(&self.dimensions, dimension)?,
            blocks: &mut self.sections,
        })
    }
}

/// The loaded sections of one dimension, read-only. A section is readable once its
/// entity carries blocks; one still generating is [`Access::Unloaded`].
pub struct DimensionView<'a, 'w, 's> {
    sections: Sections<'a>,
    blocks: &'a Query<'w, 's, &'static ChunkBlocks>,
}

/// The loaded sections of one dimension, writable. A write diverges the section from
/// every snapshot taken of it and is storage only: nothing hears of it.
pub struct DimensionViewMut<'a, 'w, 's> {
    sections: Sections<'a>,
    blocks: &'a mut Query<'w, 's, &'static mut ChunkBlocks>,
}

impl DimensionViewMut<'_, '_, '_> {
    pub fn section_entity(&self, at: SectionPos) -> Option<Entity> {
        self.sections.index.get(at)
    }
}

#[derive(Clone, Copy)]
struct Sections<'a> {
    index: &'a SectionIndex,
    layout: ColumnLayout,
}

impl<'a> Sections<'a> {
    fn of(dimensions: &'a Dimensions<'_, '_>, dimension: Entity) -> Option<Self> {
        let (index, config) = dimensions.get(dimension).ok()?;
        Some(Self {
            index,
            layout: ColumnLayout {
                min_section_y: config.min_y.div_euclid(SectionPos::SIZE as i32),
                sections: config.section_count,
            },
        })
    }

    fn bounds(self) -> BoundingBox {
        let top = self
            .layout
            .min_y()
            .saturating_add_unsigned(self.layout.height())
            .saturating_sub(1);
        BoundingBox {
            min: BlockPos::new(i32::MIN, self.layout.min_y(), i32::MIN),
            max: BlockPos::new(i32::MAX, top, i32::MAX),
        }
    }

    fn entity(self, at: SectionPos) -> Result<Entity, Unavailable> {
        if !self.layout.contains_section_y(at.y) {
            return Err(Unavailable::Outside);
        }
        self.index.get(at).ok_or(Unavailable::Unloaded)
    }

    fn access(
        self,
        at: SectionPos,
        loaded: impl FnOnce(Entity) -> bool,
        granted: Access,
    ) -> Access {
        match self.entity(at) {
            Ok(entity) if loaded(entity) => granted,
            Err(Unavailable::Outside) => Access::Outside,
            _ => Access::Unloaded,
        }
    }
}

impl Volume for DimensionView<'_, '_, '_> {
    fn bounds(&self) -> BoundingBox {
        self.sections.bounds()
    }

    fn access(&self, at: SectionPos) -> Access {
        self.sections
            .access(at, |entity| self.blocks.contains(entity), Access::Read)
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        Some(self.sections.layout)
    }
}

impl BlockVolume for DimensionView<'_, '_, '_> {
    type Section<'a>
        = PalettedSection<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        let entity = self.sections.entity(at)?;
        let blocks = self.blocks.get(entity).map_err(|_| Unavailable::Unloaded)?;
        Ok(PalettedSection(&blocks.0.0))
    }
}

impl SectionVolume for DimensionView<'_, '_, '_> {}

impl Volume for DimensionViewMut<'_, '_, '_> {
    fn bounds(&self) -> BoundingBox {
        self.sections.bounds()
    }

    fn access(&self, at: SectionPos) -> Access {
        self.sections
            .access(at, |entity| self.blocks.contains(entity), Access::Write)
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        Some(self.sections.layout)
    }
}

impl BlockVolume for DimensionViewMut<'_, '_, '_> {
    type Section<'a>
        = PalettedSection<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        let entity = self.sections.entity(at)?;
        let blocks = self.blocks.get(entity).map_err(|_| Unavailable::Unloaded)?;
        Ok(PalettedSection(&blocks.0.0))
    }
}

impl BlockVolumeMut for DimensionViewMut<'_, '_, '_> {
    type SectionMut<'a>
        = PalettedSectionMut<'a>
    where
        Self: 'a;

    fn section_mut(&mut self, at: SectionPos) -> Result<PalettedSectionMut<'_>, Denied> {
        let entity = self.sections.entity(at)?;
        let blocks = self.blocks.get_mut(entity).map_err(|_| Denied::Unloaded)?;
        Ok(PalettedSectionMut(&mut blocks.into_inner().make_mut().0))
    }
}

impl SectionVolume for DimensionViewMut<'_, '_, '_> {}

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
        let loaded = world.spawn(air_section()).id();
        let generating = world.spawn_empty().id();
        let mut index = SectionIndex::new();
        index.insert(SectionPos::new(0, 0, 0), loaded);
        index.insert(SectionPos::new(1, 0, 0), generating);
        let dimension = world.spawn((index, DimensionTypeConfig::new(0, 32))).id();

        let mut state: SystemState<WorldViewMut> = SystemState::new(&mut world);
        let mut blocks = state.get_mut(&mut world).expect("valid params");
        let mut view = blocks.get_mut(dimension).expect("a dimension");

        assert_eq!(view.access(SectionPos::new(0, 0, 0)), Access::Write);
        assert_eq!(view.access(SectionPos::new(1, 0, 0)), Access::Unloaded);
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
