use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub placed: BlockStateId,
    pub concrete: Option<BlockStateId>,
}
