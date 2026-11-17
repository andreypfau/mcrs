use bevy_ecs::component::Component;
use bevy_ecs::schedule::SystemSet;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityLight {
    pub block: u8,
    pub sky: u8,
}

impl EntityLight {
    pub const OPEN_SKY: EntityLight = EntityLight { block: 0, sky: 15 };
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostLight;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityLightSystems;
