use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::session::PlayerSession;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockActionAck {
    pub session: PlayerSession,
    pub sequence: i32,
}
