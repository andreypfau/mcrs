use bevy_ecs::component::Component;
use bevy_ecs::schedule::SystemSet;

/// The block light and the sky light at the eye of an entity, each in `0..=15`. An entity with
/// no `EntityLight` has the light [`EntityLight::OPEN_SKY`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityLight {
    pub block: u8,
    pub sky: u8,
}

impl EntityLight {
    /// Full sky light and no block light: the light of a place with no known light.
    pub const OPEN_SKY: EntityLight = EntityLight { block: 0, sky: 15 };
}

/// The systems in `PostUpdate` that write the [`EntityLight`] of an entity. A system that reads
/// the light of an entity in the same frame runs after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityLightSystems;
