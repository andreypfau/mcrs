use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;

use crate::Body;
use crate::QueryEntity;
use crate::Side;
use mcrs_minecraft_pose::Dimensions;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Physics {
    #[default]
    Collide,
    Pass,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub key: EntityUuid,
    pub width: f32,
    pub height: f32,
    pub step_height: f32,
    pub physics: Physics,
    pub side: Side,
    pub shift: bool,
    pub keeps_to_edges: bool,
    pub restitution: f64,
    pub block_bounce_factor: f32,
    pub gravity: f64,
    pub vertical_drag: f32,
    pub movement_efficiency: Option<f32>,
    pub powder_snow_solid: bool,
    pub walks_on_powder_snow: bool,
    pub intent: Option<DVec3>,
}

impl Mover {
    #[must_use]
    pub fn new(key: EntityUuid, dimensions: Dimensions, side: Side) -> Self {
        Self {
            key,
            width: dimensions.width,
            height: dimensions.height,
            step_height: 0.0,
            physics: Physics::Collide,
            side,
            shift: false,
            keeps_to_edges: false,
            restitution: 0.0,
            block_bounce_factor: 1.0,
            gravity: 0.0,
            vertical_drag: 0.98,
            movement_efficiency: None,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
            intent: None,
        }
    }

    #[must_use]
    pub fn query_entity(&self, body: &Body) -> QueryEntity {
        QueryEntity {
            key: self.key,
            position: body.position,
            fall_distance: body.fall_distance,
            shift: self.shift,
            powder_snow_solid: self.powder_snow_solid,
            walks_on_powder_snow: self.walks_on_powder_snow,
        }
    }
}
