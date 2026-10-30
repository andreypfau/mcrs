//! The vanilla commands `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` and
//! `time`, and the command effects they write.

// std: bevy_ecs needs it.
mod dimension_view;
mod execute;
mod execute_view;
mod gamemode;
#[cfg(feature = "locate")]
mod locate;
mod plugin;
mod relative_move;
mod select;
mod set;
mod teleport;
mod time;

pub use gamemode::SetGameMode;
pub use plugin::VanillaCommandsPlugin;
pub use relative_move::{DimensionChange, Pose, RelativeMove, RelativeRotation, relative_move};
pub use set::{VanillaCommands, VanillaContext, VanillaWorld};
pub use teleport::{Facing, Teleport, TeleportPlayer};
