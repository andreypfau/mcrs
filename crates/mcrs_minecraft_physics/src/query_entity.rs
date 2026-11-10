use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;

/// The entity for which a query finds colliders: the values that a context shape, the world
/// border and the entity colliders read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryEntity {
    pub key: EntityKey,
    pub position: DVec3,
    pub fall_distance: f64,
    pub shift: bool,
    /// The type flag `powder_snow_solid`: powder snow is a full cube while the fall distance is
    /// not more than 2.5.
    pub powder_snow_solid: bool,
    /// The entity can walk on powder snow: its type has the tag `powder_snow_walkable_mobs`, or
    /// it wears leather boots.
    pub walks_on_powder_snow: bool,
}

/// The kind of a collision query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QueryVariant {
    #[default]
    Collision,
    /// A placement query: scaffolding and powder snow give the empty shape, and the world border
    /// is not a collider.
    Placement,
}
