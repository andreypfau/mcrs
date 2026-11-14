use bevy_ecs::component::Component;
use bevy_ecs::schedule::SystemSet;

/// The block light and the sky light at the eye of an entity, each in `0..=15`. An entity with
/// no `EntityLight` has full sky light and no block light.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityLight {
    pub block: u8,
    pub sky: u8,
}

/// The systems in `PostUpdate` that give an entity its first [`EntityLight`]. A system that
/// reads the light of an entity in its first frame runs after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityLightSystems;
