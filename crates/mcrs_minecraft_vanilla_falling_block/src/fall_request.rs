use bevy_ecs::message::Message;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_falling_block::start;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::PendingFall;
use crate::falling_block_rules::FallingBlockRules;

/// Starts the fall of `state` at `pos` of the dimension world, whatever the block below and
/// whether the block is a gravity block (FBK-SPN-001). The sender states the state that `pos`
/// holds; the start sets the block of its fluid there. Each request becomes a [`PendingFall`] in
/// the same tick.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallRequest {
    pub pos: BlockPos,
    pub state: BlockStateId,
}

pub(crate) fn propose_falls(
    mut requests: MessageReader<FallRequest>,
    rules: Res<FallingBlockRules>,
    mut commands: Commands,
) {
    for request in requests.read() {
        commands.spawn(PendingFall(start(
            &rules.table,
            *request.pos,
            request.state,
        )));
    }
}
