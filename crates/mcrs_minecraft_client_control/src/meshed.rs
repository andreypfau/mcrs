use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Meshed(pub bool);
