use mcrs_voxel::BlockPos;

/// A write that a tick starts on the block domain. The block domain applies it later in the same
/// server tick, against the state of the block at that time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockAction {
    /// The entity steps on the block at its effect position.
    Step(BlockPos),
    /// The box of the entity passed through the block.
    Inside(BlockPos),
    /// The entity lands on the block at its effect position after a fall.
    Fall { at: BlockPos, distance: f64 },
    /// A restitution above zero changed the velocity on the block at the effect position.
    Bounce(BlockPos),
}
