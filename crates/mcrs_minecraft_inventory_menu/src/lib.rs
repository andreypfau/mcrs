//! The keys of the menu registry and the layout of the inventory menu.
// std: mcrs_minecraft_registries

mod inventory_menu;
mod menu_range;
mod menu_slot;
#[rustfmt::skip]
pub mod menu_keys;
#[rustfmt::skip]
pub mod registry_keys;

pub use inventory_menu::InventoryMenu;
pub use menu_range::MenuOrder;
pub use menu_range::MenuRange;
pub use menu_range::MenuRangeIter;
pub use menu_slot::MenuSlot;
