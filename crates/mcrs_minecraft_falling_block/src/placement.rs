use mcrs_voxel::BlockStateId;

/// The block that a landing places (FBK-LND-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The carried state, waterlogged on a water source, set at step 2.
    pub placed: BlockStateId,
    /// The default state of the concrete, set at step 5, when the concrete condition is true.
    pub concrete: Option<BlockStateId>,
}
