//! The moving entities that are not living, such as items, falling blocks, primed TNT and
//! experience orbs, as Bevy components: one parallel pass steps each of them with the physics
//! kernel over any block source and lists the entities that moved.

mod motion_hold;
mod motion_kind;
mod motion_pass_plugin;
mod motion_state;
mod motion_step;
mod movers;
mod section_cache;
mod step_outcome;
mod stirred_tiles;

pub use motion_hold::MotionHold;
pub use motion_kind::MotionKind;
pub use motion_pass_plugin::MotionPass;
pub use motion_pass_plugin::MotionPassPlugin;
pub use motion_pass_plugin::MotionRelease;
pub use motion_pass_plugin::MotionSteps;
pub use motion_state::MotionState;
pub use motion_step::MotionStep;
pub use movers::Movers;
pub use step_outcome::StepOutcome;
pub use stirred_tiles::StirredTiles;
