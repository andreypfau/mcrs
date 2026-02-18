pub mod block;
pub mod block_tags;
pub mod item;
pub mod item_tags;
pub mod loader;

pub use block::BlockTagPlugin;
pub use item::ItemTagPlugin;

pub type Tag = (&'static [&'static str], &'static [u16]);
