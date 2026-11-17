use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClientOnGround(pub bool);
