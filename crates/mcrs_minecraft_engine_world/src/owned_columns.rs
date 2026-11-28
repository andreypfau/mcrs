use std::num::NonZeroU32;

use bevy_ecs::resource::Resource;
use mcrs_engine_cell::{BoxPartition, CellIndex, ChunkKey, Leaf, LeafSize};
use mcrs_voxel::ColumnPos;

use crate::loading_rings::LOADING_RINGS;

pub const REGION_COLUMNS: NonZeroU32 = match NonZeroU32::new(32) {
    Some(width) => width,
    None => NonZeroU32::MIN,
};

#[must_use]
pub fn region_partition(cells: NonZeroU32) -> BoxPartition {
    BoxPartition::new(
        LeafSize {
            x: Leaf::Size(REGION_COLUMNS),
            y: Leaf::Whole,
            z: Leaf::Size(REGION_COLUMNS),
        },
        cells,
    )
}

#[must_use]
pub fn column_key(column: ColumnPos) -> ChunkKey {
    ChunkKey {
        x: column.x,
        y: 0,
        z: column.z,
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedColumns {
    partition: BoxPartition,
    index: CellIndex,
}

pub const HALO: u32 = LOADING_RINGS.unsigned_abs();

impl OwnedColumns {
    #[must_use]
    pub fn new(partition: BoxPartition, index: CellIndex) -> Self {
        Self { partition, index }
    }

    #[must_use]
    pub fn index(&self) -> CellIndex {
        self.index
    }

    #[must_use]
    pub fn owns(&self, column: ColumnPos) -> bool {
        self.partition.owner(column_key(column)) == self.index
    }

    #[must_use]
    pub fn reaches(&self, column: ColumnPos) -> bool {
        self.partition.reaches(self.index, column_key(column), HALO)
    }

    #[must_use]
    pub fn is_read_by_others(&self, column: ColumnPos) -> bool {
        (0..self.partition.cells().get())
            .map(CellIndex)
            .filter(|index| *index != self.index)
            .any(|index| self.partition.reaches(index, column_key(column), HALO))
    }
}

#[must_use]
pub fn owns(owned: Option<&OwnedColumns>, column: ColumnPos) -> bool {
    owned.is_none_or(|owned| owned.owns(column))
}

#[must_use]
pub fn reaches(owned: Option<&OwnedColumns>, column: ColumnPos) -> bool {
    owned.is_none_or(|owned| owned.reaches(column))
}
