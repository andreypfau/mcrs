pub mod block_state;
pub mod fluid_state;
#[rustfmt::skip]
pub mod keys;
pub mod predicate;
pub mod provider;
pub mod state_properties;

mcrs_minecraft_registries::declare_registries!(
    crate::provider::DirectBlockStateProvider => crate::keys::BLOCK_STATE_PROVIDER, World;
);
