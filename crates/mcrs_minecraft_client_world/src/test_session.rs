use mcrs_minecraft_chunk::PalettedContainer;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_voxel::SectionPos;

use crate::columns::{AIR, Column, ColumnStore, Extent, Section};

pub(crate) const FLOOR_TOP: f64 = 16.0;

/// A stone floor whose top is at [`FLOOR_TOP`], in the one column at the origin of a dimension of
/// two sections.
pub(crate) fn stone_floor(blocks: &TestBlocks) -> ColumnStore {
    let extent = Extent {
        min_section_y: 0,
        sections: 2,
    };
    let ground = Section {
        blocks: Box::new([blocks.stone; SectionPos::VOLUME]),
        biomes: PalettedContainer::Homogeneous(0),
        states: vec![AIR, blocks.stone],
    };
    let mut columns = ColumnStore::default();
    columns.enter(extent);
    columns.insert(
        ColumnPos::new(0, 0),
        Column::unlit(extent.min_section_y, vec![Some(ground), None]),
    );
    columns
}
