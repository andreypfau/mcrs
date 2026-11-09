use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;

use crate::Body;
use crate::Dimensions;
use crate::QueryEntity;
use crate::Side;

/// Whether the entity collides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Physics {
    #[default]
    Collide,
    /// No physics, for example a spectator: the move adds the displacement.
    Pass,
}

/// The values of an entity that one move reads besides its [`Body`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub key: EntityKey,
    pub width: f32,
    pub height: f32,
    pub step_height: f32,
    pub physics: Physics,
    pub side: Side,
    pub shift: bool,
    /// A player whose flight state is off: the edge back-off applies while it sneaks.
    pub keeps_to_edges: bool,
    /// The restitution: the `bounciness` attribute of a living entity, else 0.
    pub restitution: f64,
    /// The type flag `block_bounce_factor`.
    pub block_bounce_factor: f32,
    /// The effective gravity.
    pub gravity: f64,
    pub vertical_drag: f32,
    /// The `movement_efficiency` attribute of a living entity, as f32.
    pub movement_efficiency: Option<f32>,
    pub powder_snow_solid: bool,
    pub walks_on_powder_snow: bool,
    /// The intended horizontal direction of the player on its own client: the input vector
    /// rotated by the yaw, for the minor collision flag.
    pub intent: Option<DVec3>,
}

impl Mover {
    /// A mover that collides, with no step height, restitution, gravity, movement efficiency,
    /// powder snow rule or intent, a block bounce factor of 1 and a vertical drag of `0.98F`.
    #[must_use]
    pub fn new(key: EntityKey, dimensions: Dimensions, side: Side) -> Self {
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

    /// The entity for a query of colliders, with the state of `body`.
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
