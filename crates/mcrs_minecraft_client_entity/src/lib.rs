//! The packet side of the client entities that the server adds: the packets that add, move and
//! change an entity, and the network id of each entity.
// std: bevy_ecs

mod apply_entity_data;
pub mod delta;
mod network_entities;
mod plugin;
mod receive;

pub use apply_entity_data::*;
pub use network_entities::*;
pub use plugin::*;
pub use receive::{DEATH_EVENT, WORLD_LIMIT};
