use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_player_holders::HandAction;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandInput {
    pub viewer: Entity,
    pub action: HandAction,
}
