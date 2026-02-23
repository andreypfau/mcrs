use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_engine::world::block::BlockPos;
use mcrs_protocol::BlockStateId;

#[derive(Clone, Copy, Debug, Message)]
pub struct PlayerWillDestroyBlock {
    pub player: Entity,
    pub chunk: Entity,
    pub block_pos: BlockPos,
    pub block_state: BlockStateId,
}
