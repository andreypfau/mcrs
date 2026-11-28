use bevy_ecs::message::Message;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Res;
use mcrs_minecraft_falling_block::start;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::PendingFall;
use crate::falling_block_rules::FallingBlockRules;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallRequest {
    pub pos: BlockPos,
    pub state: BlockStateId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arrival {
    order: usize,
    request: FallRequest,
}

pub(crate) fn propose_falls(
    mut requests: MessageReader<FallRequest>,
    rules: Res<FallingBlockRules>,
    mut commands: Commands,
    mut ordered: Local<Vec<Arrival>>,
) {
    ordered.clear();
    let arrivals = requests.read().enumerate().map(|(order, request)| Arrival {
        order,
        request: *request,
    });
    ordered.extend(arrivals);
    ordered.sort_unstable_by_key(|arrival| (arrival.request.pos.to_array(), arrival.order));
    ordered.dedup_by_key(|arrival| arrival.request.pos);
    for Arrival { request, .. } in ordered.iter() {
        commands.spawn(PendingFall(start(
            &rules.table,
            *request.pos,
            request.state,
        )));
    }
}
