use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Thrower(pub u128);
