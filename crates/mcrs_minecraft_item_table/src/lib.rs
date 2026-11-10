//! The prototype of each item kind and the effective components of an item stack.
// std: mcrs_minecraft_item_stack

extern crate alloc;

mod component_row;
mod duplicate_key;
mod item_kind;
mod item_registry;
mod item_table;
mod prototype;
mod row_hashing;

pub use component_row::ComponentRow;
pub use duplicate_key::DuplicateKey;
pub use item_kind::item_kind;
pub use item_registry::ItemRegistry;
pub use item_table::ItemTable;
pub use prototype::Prototype;
pub use row_hashing::RowHashing;
