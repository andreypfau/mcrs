use mcrs_voxel::{BlockPos, BlockStateId};

use crate::location_view::LocationView;

/// A host without a world: each value is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWorld;

impl LocationView for NoWorld {
    fn block_state(&self, _: BlockPos) -> Option<BlockStateId> {
        None
    }
}
