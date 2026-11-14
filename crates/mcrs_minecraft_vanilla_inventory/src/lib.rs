//! The four holders of each player (INV-SLT-002): built from the player file and the carried
//! values at each apply, written back at each capture, changed by the clicks of the inventory
//! menu, and kept in agreement with the client.
// std: mcrs_minecraft_engine

mod apply_click;
mod apply_inventory_input;
mod capture_holders;
mod carried_codec;
mod claimed_slot;
mod click_drag;
mod click_menu;
mod click_slots;
mod close_container;
mod close_inventory_menu;
mod creative_drops;
mod decode_click;
mod forget_rejected_swap;
mod full_sends;
mod hand_action;
mod hand_launch;
mod holder_slots;
mod insert_stack;
mod inventory_input;
mod inventory_menu_sync;
mod inventory_persistence_plugin;
mod inventory_set;
mod kept_equipment;
mod launch_drops;
mod menu_content;
mod menu_queue;
mod player_file;
mod player_file_codec;
mod player_holders;
mod player_stacks;
mod request_ids;
mod restore_holders;
mod saved_stack;
mod send_full_state;
mod sent_stack;
mod set_carried_item;
mod set_creative_mode_slot;
mod sync_menus;
mod vanilla_inventory_plugin;

pub use claimed_slot::ClaimedSlot;
pub use hand_action::HandAction;
pub use inventory_input::InventoryInput;
pub use inventory_menu_sync::InventoryMenuSync;
pub use inventory_persistence_plugin::InventoryPersistencePlugin;
pub use inventory_set::InventorySet;
pub use player_holders::PlayerHolders;
pub use vanilla_inventory_plugin::VanillaInventoryPlugin;
