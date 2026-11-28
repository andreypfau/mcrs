use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryEntity {
    pub key: EntityUuid,
    pub position: DVec3,
    pub fall_distance: f64,
    pub shift: bool,
    pub powder_snow_solid: bool,
    pub walks_on_powder_snow: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QueryVariant {
    #[default]
    Collision,
    Placement,
}
