use bevy_ecs::component::Component;
use mcrs_minecraft_physics::Birth;

/// The birth of an entity: its order key and its phase offset. Set at the spawn, kept across a
/// change of cell or dimension, and saved with the entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityBirth(pub Birth);
