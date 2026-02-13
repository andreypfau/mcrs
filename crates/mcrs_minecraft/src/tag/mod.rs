pub mod block;
pub mod item;
mod loader;

pub use block::BlockTagPlugin;
pub use item::ItemTagPlugin;

pub type Tag = (&'static [&'static str], &'static [u16]);
