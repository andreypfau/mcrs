//! The moving entities that are not living, such as items, falling blocks, primed TNT and
//! experience orbs, as Bevy components: one parallel pass steps each of them with the physics
//! kernel and lists the entities that moved.

mod entity_motion_plugin;
mod motion_hold;
mod motion_kind;
mod motion_state;
mod movers;
mod section_cache;
mod step_motion;
mod step_outcome;
mod stirred_tiles;

pub use entity_motion_plugin::EntityMotionPlugin;
pub use entity_motion_plugin::MotionPass;
pub use entity_motion_plugin::MotionRelease;
pub use motion_hold::MotionHold;
pub use motion_kind::MotionKind;
pub use motion_state::MotionState;
pub use movers::Movers;
pub use step_outcome::StepOutcome;
