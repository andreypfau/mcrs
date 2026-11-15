//! An NBT path read from text, and its selection, get-or-create and set on NBT tags.

// std: mcrs_minecraft_nbt

extern crate alloc;

mod nbt_path;
mod nbt_path_error;
mod nbt_path_operation_error;
mod node;
mod number;
mod parse;
mod selected_tag;

pub use nbt_path::NbtPath;
pub use nbt_path_error::NbtPathError;
pub use nbt_path_operation_error::NbtPathOperationError;
pub use selected_tag::SelectedTag;
