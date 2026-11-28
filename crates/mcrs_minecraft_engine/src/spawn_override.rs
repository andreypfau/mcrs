use bevy_ecs::resource::Resource;
use bevy_math::DVec3;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SpawnOverride(pub DVec3);
