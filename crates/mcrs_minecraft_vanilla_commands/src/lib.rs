//! The vanilla commands `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` and
//! `time`, and the command effects they write.

// std: bevy_ecs, mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the worlds, the
// identifiers and the command packets, need it.
mod dimension_view;
mod execute;
mod execute_view;
mod gamemode;
mod locate;
mod plugin;
mod select;
mod teleport;
mod time;

pub use gamemode::SetGameMode;
pub use plugin::VanillaCommandsPlugin;
pub use teleport::{Facing, Teleport, TeleportPlayer};
