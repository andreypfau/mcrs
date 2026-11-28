use crate::{MalusTable, MobFlags};
use mcrs_voxel::BlockPos;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PathInputs {
    pub width: f32,
    pub height: f32,
    pub malus: MalusTable,
    pub block_position: BlockPos,
    pub flags: MobFlags,
}

impl PathInputs {
    #[must_use]
    pub fn new(block_position: BlockPos, width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            malus: MalusTable::default(),
            block_position,
            flags: MobFlags::default(),
        }
    }
}
