//! The packet side of the client entities that the server adds: the packets that add, move and
//! change an entity, and the network id of each entity.
// std: bevy_ecs

mod apply_entity_data;
mod attribute_instances;
pub mod delta;
mod entity_light;
mod network_entities;
mod plugin;
mod receive;

pub use apply_entity_data::*;
pub use attribute_instances::AttributeInstances;
pub use network_entities::*;
pub use plugin::*;
pub use receive::{DEATH_EVENT, WORLD_LIMIT};
