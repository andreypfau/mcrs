use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockAction {
    Step(BlockPos),
    Inside(BlockPos),
    Fall { at: BlockPos, distance: f64 },
    Bounce(BlockPos),
}
