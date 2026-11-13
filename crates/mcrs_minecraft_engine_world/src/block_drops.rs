//! The items that the blocks of a dimension leave when the block simulation destroys them.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_item::ItemStack;
use mcrs_voxel::{BlockPos, BlockStateId};

/// What a block leaves when the block simulation destroys it with its drops. The rules of the
/// blocks supply the items: the loot of each block is theirs.
pub trait BlockDrops {
    /// Adds to `drops` the items that `block` leaves; `block_at` reads the world at the time of
    /// the drop.
    fn drops(
        &self,
        _block: &DroppingBlock,
        _block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        _drops: &mut Vec<ItemStack>,
    ) {
    }
}

/// A block that leaves its items, with what its drops can depend on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DroppingBlock {
    pub at: BlockPos,
    pub state: BlockStateId,
    /// The stack that broke the block; `None` is the empty stack.
    pub tool: Option<ItemStack>,
    /// The player that broke the block; `None` for a block that a change around it removed.
    pub breaker: Option<Entity>,
    pub tick: u64,
    pub world_seed: u64,
}

/// An item that a block left at `at` when the block simulation destroyed it with its drops, in
/// the dimension world of the block.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemDropped {
    pub at: BlockPos,
    pub stack: ItemStack,
}

/// A survival break of `state` at `at`, before its write applies; the hit part drops only when the
/// break `harvests` it. The break also removes `other`, the other part of a block of two parts, as
/// a change around it once the hit part is `left`.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockBroken {
    pub at: BlockPos,
    pub state: BlockStateId,
    pub left: BlockStateId,
    pub other: Option<(BlockPos, BlockStateId)>,
    pub harvests: bool,
    pub breaker: Entity,
}
