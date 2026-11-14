//! The moving entities that are not living, such as items, falling blocks, primed TNT and
//! experience orbs, as Bevy components: one parallel pass steps each of them with the physics
//! kernel, and their changes become tile events.

mod appearance;
mod assign_ids;
mod departures;
mod entity_motion_plugin;
mod motion_id;
mod motion_kind;
mod motion_state;
mod publish_motion;
mod published_tile;
mod section_cache;
mod step_motion;
mod step_outcome;
mod stirred_tiles;
mod subject;

pub use appearance::Appearance;
pub use entity_motion_plugin::EntityMotionPlugin;
pub use entity_motion_plugin::MotionPass;
pub use entity_motion_plugin::MotionPublish;
pub use motion_id::MotionId;
pub use motion_kind::MotionKind;
pub use motion_state::MotionState;
pub use published_tile::PublishedTile;
pub use step_outcome::StepOutcome;
