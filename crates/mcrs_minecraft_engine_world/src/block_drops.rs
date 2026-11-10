//! The items that the blocks of a dimension leave when the block simulation destroys them.

use bevy_ecs::message::Message;
use mcrs_minecraft_item::ItemStack;
use mcrs_voxel::{BlockPos, BlockStateId};

/// What a block leaves when the block simulation destroys it with its drops. The rules of the
/// blocks supply the items: the loot of each block is theirs.
pub trait BlockDrops {
    /// Adds to `drops` the items that `state` leaves when the simulation destroys it at `at`.
    fn drops(&self, _at: BlockPos, _state: BlockStateId, _drops: &mut Vec<ItemStack>) {}
}

/// An item that a block left at `at` when the block simulation destroyed it with its drops, in
/// the dimension world of the block.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemDropped {
    pub at: BlockPos,
    pub stack: ItemStack,
}
