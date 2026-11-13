//! The four holders of each player (INV-SLT-002): built from the player file and the carried
//! values at each apply, and written back at each capture.
// std: mcrs_minecraft_engine

mod capture_holders;
mod carried_codec;
mod close_inventory_menu;
mod full_sends;
mod insert_stack;
mod inventory_menu_sync;
mod inventory_persistence_plugin;
mod inventory_set;
mod kept_equipment;
mod player_file;
mod player_file_codec;
mod player_holders;
mod player_stacks;
mod restore_holders;
mod saved_stack;
mod send_full_state;
mod set_carried_item;
mod vanilla_inventory_plugin;

pub use inventory_menu_sync::InventoryMenuSync;
pub use inventory_persistence_plugin::InventoryPersistencePlugin;
pub use inventory_set::InventorySet;
pub use player_holders::PlayerHolders;
pub use vanilla_inventory_plugin::VanillaInventoryPlugin;
