// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod block_predicate_type;
pub mod block_state_provider;
pub mod block_state_provider_type;

pub use block_predicate_type::BlockPredicateType;
pub use block_state_provider_type::BlockStateProviderType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const BLOCK_PREDICATE_TYPE: RegistryKey<crate::keys::BlockPredicateType> = RegistryKey::new(rl!("minecraft:block_predicate_type"));
impl Registered for crate::keys::BlockPredicateType {
    const REGISTRY: RegistryKey<Self> = BLOCK_PREDICATE_TYPE;
}

pub const BLOCK_STATE_PROVIDER: RegistryKey<crate::provider::DirectBlockStateProvider> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider"));
impl Registered for crate::provider::DirectBlockStateProvider {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER;
}

pub const BLOCK_STATE_PROVIDER_TYPE: RegistryKey<crate::keys::BlockStateProviderType> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider_type"));
impl Registered for crate::keys::BlockStateProviderType {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER_TYPE;
}

pub fn bindings() -> [TypeBinding; 3] {
    [
        BLOCK_PREDICATE_TYPE.binding(),
        BLOCK_STATE_PROVIDER.binding(),
        BLOCK_STATE_PROVIDER_TYPE.binding(),
    ]
}
