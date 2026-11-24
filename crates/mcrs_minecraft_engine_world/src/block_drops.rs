use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_voxel::{BlockPos, BlockStateId};

pub trait BlockDrops {
    fn drops(
        &self,
        _block: &DroppingBlock,
        _block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        _drops: &mut Vec<ItemStack>,
    ) {
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DroppingBlock {
    pub at: BlockPos,
    pub state: BlockStateId,
    pub tool: Option<ItemStack>,
    pub breaker: Option<Entity>,
    pub tick: u64,
    pub world_seed: u64,
}

#[derive(Message, Clone, Debug, PartialEq)]
pub struct ItemDropped {
    pub at: BlockPos,
    pub stack: ItemStack,
}

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockBroken {
    pub at: BlockPos,
    pub state: BlockStateId,
    pub left: BlockStateId,
    pub other: Option<(BlockPos, BlockStateId)>,
    pub harvests: bool,
    pub breaker: Entity,
}
