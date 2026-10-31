use bevy_ecs::resource::Resource;

/// The seed every dimension's noise router is compiled against.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldSeed(pub u64);
