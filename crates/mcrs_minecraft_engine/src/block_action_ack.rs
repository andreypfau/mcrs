use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::session::PlayerSession;

/// The acknowledgement of the block actions of one player in one tick, with the highest sequence
/// of the tick. The muxer sends it after the column changes of the tick, so the client gets the
/// changed block states first.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockActionAck {
    pub session: PlayerSession,
    pub sequence: i32,
}
