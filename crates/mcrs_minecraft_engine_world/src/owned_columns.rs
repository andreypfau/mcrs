use std::num::NonZeroU32;

use bevy_ecs::resource::Resource;
use mcrs_engine_cell::{BoxPartition, CellIndex, ChunkKey, Leaf, LeafSize};
use mcrs_voxel::{BlockPos, ColumnPos};

use crate::world::lifecycle::column_loading::LOADING_RINGS;

/// The width of a region file in columns: the minimum leaf of a cell on X and Z.
pub const REGION_COLUMNS: NonZeroU32 = match NonZeroU32::new(32) {
    Some(width) => width,
    None => NonZeroU32::MIN,
};

/// The static division of the columns of a world into regions, over `cells` cells. A cell owns
/// whole columns: the minimum leaf on Y is the height of the world.
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

/// The key of `column` in a partition of columns.
#[must_use]
pub fn column_key(column: ColumnPos) -> ChunkKey {
    ChunkKey {
        x: column.x,
        y: 0,
        z: column.z,
    }
}

/// The columns that this world owns, when its world has more than one cell. Only the owner of a
/// column changes it; the other columns that the world holds are its halo, a read-only copy
/// that the owner keeps up to date with messages. Without this resource the world owns every
/// column.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedColumns {
    partition: BoxPartition,
    index: CellIndex,
}

/// The columns past its boxes whose tickets a cell raises: the loading rings of a ticket, so a
/// column at the edge of a box is lit with all its neighbours, as in a world of one cell.
const HALO: u32 = LOADING_RINGS.unsigned_abs();

impl OwnedColumns {
    /// The columns of the cell `index` in `partition`, which raises the tickets of the columns
    /// within the loading rings of its boxes.
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
    pub fn owns_block(&self, pos: BlockPos) -> bool {
        self.owns(ColumnPos::from(pos))
    }

    /// Whether `column` is within the halo of the boxes of this cell.
    #[must_use]
    pub fn reaches(&self, column: ColumnPos) -> bool {
        self.partition.reaches(self.index, column_key(column), HALO)
    }

    /// Whether another cell may read `column`: a cell reads the columns of its halo, and the
    /// columns next to a column that it lights in cold space.
    #[must_use]
    pub fn is_read_by_others(&self, column: ColumnPos) -> bool {
        (0..self.partition.cells().get())
            .map(CellIndex)
            .filter(|index| *index != self.index)
            .any(|index| self.partition.reaches(index, column_key(column), HALO))
    }
}

/// Whether the world of `owned` owns `column`: every column when it is the only cell.
#[must_use]
pub fn owns(owned: Option<&OwnedColumns>, column: ColumnPos) -> bool {
    owned.is_none_or(|owned| owned.owns(column))
}

/// Whether the world of `owned` raises a ticket on `column`: every column when it is the only
/// cell.
#[must_use]
pub fn reaches(owned: Option<&OwnedColumns>, column: ColumnPos) -> bool {
    owned.is_none_or(|owned| owned.reaches(column))
}
