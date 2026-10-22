#[cfg(all(
    feature = "bevy",
    feature = "test-support",
    not(target_family = "wasm")
))]
mod corpus;
pub mod definitions;

#[cfg(all(
    feature = "bevy",
    feature = "test-support",
    not(target_family = "wasm")
))]
pub use corpus::test_corpus;
