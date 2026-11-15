//! The plugins of a headless minigame over the player inventory, one plugin for each feature.

mod enter_game;
mod kit_plugin;
mod kit_snapshot;
mod leave_game;

pub use enter_game::EnterGame;
pub use kit_plugin::KitPlugin;
pub use kit_snapshot::KitSnapshot;
pub use leave_game::LeaveGame;
