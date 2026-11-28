use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryPressed {
    pub viewer: Entity,
    pub menu: Entity,
    pub entry: u16,
    pub button: u8,
}
