use bevy_ecs::event::Event;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_text::Text;

#[derive(Event, Clone, Debug)]
pub struct OperatorBroadcast {
    pub source: PlayerSession,
    pub message: Text,
}
