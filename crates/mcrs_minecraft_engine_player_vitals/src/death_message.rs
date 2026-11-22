use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_text::Text;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct DeathMessage {
    pub player: Entity,
    pub text: Text,
}
