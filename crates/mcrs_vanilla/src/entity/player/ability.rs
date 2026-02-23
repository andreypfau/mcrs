use bevy_ecs::prelude::Component;

#[derive(Component, Default, Debug, Clone, Copy)]
#[component(storage = "SparseSet")]
pub struct InstantBuild;
