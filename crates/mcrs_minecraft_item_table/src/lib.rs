//! The prototype of each item kind and the effective components of an item stack.
// std: mcrs_minecraft_item_stack

extern crate alloc;

mod item_kind;
mod item_table;
mod prototype;

pub use item_kind::item_kind;
pub use item_table::ItemTable;
pub use prototype::Prototype;
