//! The stacks of a player and its selected slot as values, read from and written to the keys
//! `Inventory`, `SelectedItemSlot` and `equipment` of the player file (INV-SAV-001, INV-SAV-002).
// std: mcrs_minecraft_item_stack

mod equipment_key;
mod file_error;
mod player_file;
mod player_inventory_file;
mod player_items;
mod read;
mod write;

pub use file_error::FileError;
pub use player_inventory_file::PlayerInventoryFile;
pub use player_items::PlayerItems;
pub use read::read;
pub use write::write;
