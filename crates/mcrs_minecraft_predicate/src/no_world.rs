use mcrs_voxel::{BlockPos, BlockStateId};

use crate::entity_view::EntityView;
use crate::location_view::LocationView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWorld;

impl LocationView for NoWorld {
    fn block_state(&self, _: BlockPos) -> Option<BlockStateId> {
        None
    }
}

impl EntityView for NoWorld {
    type Entity = ();
}
