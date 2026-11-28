use core::marker::PhantomData;

use mcrs_voxel::{BlockPos, BlockStateId};

use crate::entity_view::EntityView;
use crate::location_view::LocationView;

pub struct BlockHost<F, E = ()> {
    lookup: F,
    entity: PhantomData<fn() -> E>,
}

pub(crate) type NoBlocks = fn(BlockPos) -> Option<BlockStateId>;

impl<F: Fn(BlockPos) -> Option<BlockStateId>, E> BlockHost<F, E> {
    #[must_use]
    pub const fn new(lookup: F) -> Self {
        Self {
            lookup,
            entity: PhantomData,
        }
    }
}

impl BlockHost<NoBlocks> {
    #[must_use]
    pub const fn none() -> Self {
        Self::new(no_block)
    }
}

fn no_block(_: BlockPos) -> Option<BlockStateId> {
    None
}

impl<F: Fn(BlockPos) -> Option<BlockStateId>, E> LocationView for BlockHost<F, E> {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        (self.lookup)(pos)
    }
}

impl<F, E: Copy + PartialEq> EntityView for BlockHost<F, E> {
    type Entity = E;
}
