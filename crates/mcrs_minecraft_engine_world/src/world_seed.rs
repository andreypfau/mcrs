use bevy_ecs::resource::Resource;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldSeed(pub u64);
