//! The items that the blocks of a dimension leave when the block simulation destroys them.

use bevy_ecs::message::{Message, MessageWriter};
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_simulation::Destroyed;
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

/// Asks `rules` for the items of each block of `destroyed` that drops them.
pub(crate) fn drop_items<B: BlockDrops>(
    rules: &B,
    destroyed: &[Destroyed],
    scratch: &mut Vec<ItemStack>,
    dropped: &mut MessageWriter<ItemDropped>,
) {
    for block in destroyed {
        if !block.drops {
            continue;
        }
        rules.drops(block.pos, block.state, scratch);
        for stack in scratch.drain(..) {
            dropped.write(ItemDropped {
                at: block.pos,
                stack,
            });
        }
    }
}
