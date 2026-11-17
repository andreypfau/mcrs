use bevy_ecs::resource::Resource;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BirthClock(pub u64);
