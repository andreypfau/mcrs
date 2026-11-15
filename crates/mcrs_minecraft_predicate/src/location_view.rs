use mcrs_voxel::{BlockPos, BlockStateId};

/// The world as a location predicate reads it. `None` means that the host does not have the
/// value, for example at a position that is not loaded.
pub trait LocationView {
    /// The block state at `pos`.
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId>;
}
