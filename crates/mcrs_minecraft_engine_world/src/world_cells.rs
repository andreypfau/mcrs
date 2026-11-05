use std::num::NonZeroU32;

use bevy_ecs::prelude::Resource;

/// How the host divides each dimension world into cells. A world of one cell, the default,
/// owns every column. A world of more cells divides its columns by region over the cells, and
/// each cell raises the tickets of the columns within the loading rings of its regions.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldCells {
    pub cells: NonZeroU32,
}

impl Default for WorldCells {
    fn default() -> Self {
        Self {
            cells: NonZeroU32::MIN,
        }
    }
}
