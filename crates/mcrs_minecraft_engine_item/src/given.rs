use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

/// Where the items of a `give` or a `give_or_drop` went.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Given {
    pub holder: Entity,
    /// The first stack of `holder` that took items, or `None` when no item found room.
    pub stack: Option<Entity>,
    /// The stack of the items that found no room. After `give` it keeps the placement that it had,
    /// which is none for a given value; after `give_or_drop` it is dropped. Its owner places it or
    /// despawns it.
    pub remainder: Option<Entity>,
}
