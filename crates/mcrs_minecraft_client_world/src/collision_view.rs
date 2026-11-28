use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_volume::DenseSection;
use mcrs_minecraft_volume::SectionBlocks;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::columns::Columns;
use crate::columns::Extent;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientBlocks<'a, C> {
    columns: &'a C,
    tables: &'a Tables,
}

impl<'a, C: Columns> ClientBlocks<'a, C> {
    #[must_use]
    pub(crate) fn new(columns: &'a C, tables: &'a Tables) -> Self {
        Self { columns, tables }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientSection<'a> {
    blocks: DenseSection<'a>,
    masks: SectionMasks,
}

impl<C: Columns> CollisionWorld for ClientBlocks<'_, C> {
    type Section<'s>
        = ClientSection<'s>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        let Some(extent) = self.columns.extent() else {
            return SectionLookup::Outside;
        };
        if !spans(extent, at.y) {
            return SectionLookup::Outside;
        }
        let Some(column) = self.columns.column(at.x, at.z) else {
            return SectionLookup::Unloaded;
        };
        let section = match column.section(at.y) {
            Some(stored) => ClientSection {
                blocks: DenseSection(&stored.blocks),
                masks: SectionMasks::from_palette(
                    &self.tables.collision,
                    stored.states.iter().copied(),
                ),
            },
            None => ClientSection {
                blocks: DenseSection::air(),
                masks: SectionMasks::EMPTY,
            },
        };
        SectionLookup::Loaded(section)
    }

    fn border(&self) -> Option<&WorldBorder> {
        None
    }
}

impl CollisionSection for ClientSection<'_> {
    fn masks(&self) -> Option<&SectionMasks> {
        Some(&self.masks)
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.blocks.get(at)
    }
}

fn spans(extent: Extent, section_y: i32) -> bool {
    section_y
        .checked_sub(extent.min_section_y)
        .and_then(|offset| usize::try_from(offset).ok())
        .is_some_and(|offset| offset < extent.sections)
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_physics::TestBlocks;

    use super::*;
    use crate::test_session::stone_floor;

    #[test]
    fn a_section_reads_as_outside_or_unloaded_by_its_place() {
        let blocks = TestBlocks::new();
        let tables = Tables::new(blocks.table.clone());
        let store = stone_floor(&blocks);
        let world = ClientBlocks::new(&store, &tables);
        for (at, expected) in [
            (SectionPos::new(0, -1, 0), SectionLookup::Outside),
            (SectionPos::new(0, 2, 0), SectionLookup::Outside),
            (SectionPos::new(1, 0, 0), SectionLookup::Unloaded),
        ] {
            let read: SectionLookup<()> = match world.section(at) {
                SectionLookup::Outside => SectionLookup::Outside,
                SectionLookup::Unloaded => SectionLookup::Unloaded,
                SectionLookup::Loaded(_) => SectionLookup::Loaded(()),
            };
            assert_eq!(read, expected, "{at:?}");
        }
    }
}
