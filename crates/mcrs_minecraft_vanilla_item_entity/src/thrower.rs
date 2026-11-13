use bevy_ecs::component::Component;

/// The UUID of the entity that threw the stack. It limits neither a pickup nor a merge.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Thrower(pub u128);
