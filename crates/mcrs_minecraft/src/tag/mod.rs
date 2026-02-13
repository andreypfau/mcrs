pub mod block;
pub mod item;
mod loader;

pub type Tag = (&'static [&'static str], &'static [u16]);
