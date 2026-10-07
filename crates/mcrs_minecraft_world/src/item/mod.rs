#[cfg(feature = "bevy")]
mod corpus;
pub mod definitions;
pub mod enchantments;
pub mod tool;

#[cfg(feature = "bevy")]
pub use corpus::test_corpus;
pub use enchantments::{test_enchantment_effects, test_enchantment_registry};
