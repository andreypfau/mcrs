use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct YzxPos {
    pub y: i32,
    pub z: i32,
    pub x: i32,
}

impl From<BlockPos> for YzxPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            y: pos.y,
            z: pos.z,
            x: pos.x,
        }
    }
}
