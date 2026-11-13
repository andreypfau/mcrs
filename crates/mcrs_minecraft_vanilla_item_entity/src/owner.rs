use bevy_ecs::component::Component;

/// The UUID of the only player that can pick up the item entity (IEN-PCK-002).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Owner(pub u128);
