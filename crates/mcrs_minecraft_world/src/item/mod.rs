#[cfg(all(
    feature = "bevy",
    feature = "test-support",
    not(target_family = "wasm")
))]
mod corpus;
pub mod definitions;
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub mod enchantments;
pub mod tool;

#[cfg(all(
    feature = "bevy",
    feature = "test-support",
    not(target_family = "wasm")
))]
pub use corpus::test_corpus;
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub use enchantments::{test_enchantment_effects, test_enchantment_registry};
