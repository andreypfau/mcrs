//! The vanilla commands `execute`, `gamemode`, `teleport` with its alias `tp`, `locate`, `summon`,
//! `kill`, `damage`, `data get entity`, `time`, `op` and `deop`, and the command effects they write.

// std: bevy_ecs needs it.
mod ban;
mod damage;
mod data;
mod dimension_view;
mod execute;
mod execute_view;
mod gamemode;
mod kill;
#[cfg(feature = "locate")]
mod locate;
mod mob_hit;
mod mob_view;
mod op;
mod plugin;
mod relative_move;
mod select;
mod set;
mod summon;
mod teleport;
mod time;
mod whitelist;

pub use gamemode::SetGameMode;
pub use mob_hit::HitMob;
pub use plugin::VanillaCommandsPlugin;
pub use relative_move::{DimensionChange, Pose, RelativeMove, RelativeRotation, relative_move};
pub use set::{VanillaCommands, VanillaContext, VanillaWorld};
pub use teleport::{Facing, Teleport, TeleportPlayer};
