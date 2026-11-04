use std::num::NonZeroU32;

use bevy_ecs::prelude::Resource;

use crate::world::lifecycle::column_loading::LOADING_RINGS;

/// How the host divides each dimension world into cells. A world of one cell, the default,
/// owns every column. A world of more cells divides its columns by region over the cells, and
/// each cell raises the tickets of the columns within `halo` columns of its regions.
///
/// A halo narrower than the loading rings of a ticket lights the columns at the edge of a
/// region without all their neighbours, so those columns differ from a world of one cell.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldCells {
    pub cells: NonZeroU32,
    pub halo: u32,
}

impl Default for WorldCells {
    fn default() -> Self {
        Self {
            cells: NonZeroU32::MIN,
            halo: LOADING_RINGS.unsigned_abs(),
        }
    }
}
