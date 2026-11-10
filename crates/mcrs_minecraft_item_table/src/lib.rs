//! The prototype of each item kind and the effective components of an item stack.
// std: mcrs_minecraft_item_stack

extern crate alloc;

mod bake;
mod bake_error;
mod component_row;
mod definition_file;
mod duplicate_key;
mod item_definition;
mod item_names;
mod item_registry;
mod item_table;
mod prototype;
mod row_hashing;
mod skipped_type;

pub use bake::BakedItems;
pub use bake::bake;
pub use bake_error::BakeError;
pub use component_row::ComponentRow;
pub use definition_file::DefinitionFile;
pub use duplicate_key::DuplicateKey;
pub use item_registry::ItemRegistry;
pub use item_table::ItemTable;
pub use prototype::Prototype;
pub use row_hashing::RowHashing;
pub use skipped_type::SkippedType;
