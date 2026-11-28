//! The plugins of a headless minigame over the player inventory, one plugin for each feature.

mod arena_rules_plugin;
mod enter_game;
mod frozen;
mod kit_plugin;
mod kit_snapshot;
mod leave_game;
mod open_shop;
mod shop_plugin;

pub use arena_rules_plugin::ArenaRules;
pub use arena_rules_plugin::ArenaRulesPlugin;
pub use enter_game::EnterGame;
pub use frozen::Frozen;
pub use kit_plugin::KitPlugin;
pub use kit_snapshot::KitSnapshot;
pub use leave_game::LeaveGame;
pub use open_shop::OpenShop;
pub use shop_plugin::ShopPlugin;
