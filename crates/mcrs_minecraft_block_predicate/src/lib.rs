pub mod block_state;
#[rustfmt::skip]
pub mod keys;
pub mod predicate;
pub mod provider;

mcrs_minecraft_registries::read_with_serde!(crate::provider::DirectBlockStateProvider);
