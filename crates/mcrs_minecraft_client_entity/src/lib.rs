//! The packet side of the client entities that the server adds: the packets that add, move and
//! change an entity, and the network id of each entity.
// std: bevy_ecs

mod add_entity_data;
mod apply_entity_data;
pub mod delta;
mod entity_light;
mod moved_entity;
mod network_entities;
mod orb_interpolation;
mod plugin;
mod project_simulated_positions;
mod receive;
mod simulated_motion;
mod velocity;

pub use add_entity_data::*;
pub use apply_entity_data::*;
pub use network_entities::*;
pub use orb_interpolation::OrbInterpolation;
pub use plugin::*;
pub use receive::{DEATH_EVENT, WORLD_LIMIT};
pub use velocity::{Approach, Velocity};
