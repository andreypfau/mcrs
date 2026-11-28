use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct EntityTicking;
