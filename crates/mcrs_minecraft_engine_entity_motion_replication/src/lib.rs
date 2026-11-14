//! The replication of the moving entities that are not living: each one gets a network id before
//! it moves, and its spawn, its moves and its removal become tile events.

mod appearance;
mod assign_ids;
mod departures;
mod entity_motion_replication_plugin;
mod hold_unseen;
mod motion_id;
mod publish_motion;
mod published_tile;
mod subject;

pub use appearance::Appearance;
pub use entity_motion_replication_plugin::EntityMotionReplicationPlugin;
pub use entity_motion_replication_plugin::MotionPublish;
pub use motion_id::MotionId;
pub use published_tile::PublishedTile;
