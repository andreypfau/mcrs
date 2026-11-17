#![no_std]

extern crate alloc;

mod armour_gates;
mod armour_place;
mod armour_take;
mod locking_enchantments;
mod player_entity_type;
mod player_holders;
mod player_holders_plugin;
mod player_policy;
mod restore_holders;

pub use player_holders::PlayerHolders;
pub use player_holders_plugin::PlayerHoldersPlugin;
pub use restore_holders::restore_carried;
pub use restore_holders::restore_holders;
