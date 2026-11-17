use std::num::NonZeroU32;

use bevy_ecs::prelude::Resource;

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
