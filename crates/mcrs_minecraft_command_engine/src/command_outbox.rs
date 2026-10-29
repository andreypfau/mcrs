use crate::effect_queue::ApplyNext;
use crate::operator_broadcast::OperatorBroadcast;
use crate::source_state::SourceState;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;

/// What one run of a handler leaves for the host: its effects in the order of writing, then its
/// messages. The host lends it to the world the handler runs in and empties it after the run.
#[derive(Resource, Debug, Default)]
pub(crate) struct CommandOutbox {
    pub(crate) effects: Vec<ApplyNext>,
    pub(crate) packets: Vec<OutboundPlayerPacket>,
    pub(crate) broadcasts: Vec<OperatorBroadcast>,
    pub(crate) forked: Vec<SourceState>,
}
