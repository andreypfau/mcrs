use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::Message;

#[derive(Message, EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuOpened {
    #[event_target]
    pub viewer: Entity,
    pub menu: Entity,
}
