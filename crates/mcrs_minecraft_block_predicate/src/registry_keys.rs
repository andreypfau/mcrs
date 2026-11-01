// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_PREDICATE_TYPE: RegistryKey<crate::block_predicate_type_keys::BlockPredicateType> =
    RegistryKey::new(rl!("minecraft:block_predicate_type"));
impl Registered for crate::block_predicate_type_keys::BlockPredicateType {
    const REGISTRY: RegistryKey<Self> = BLOCK_PREDICATE_TYPE;
}

pub const BLOCK_STATE_PROVIDER: RegistryKey<crate::provider::DirectBlockStateProvider> =
    RegistryKey::new(rl!("minecraft:worldgen/block_state_provider"));
impl Registered for crate::provider::DirectBlockStateProvider {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER;
}

pub const BLOCK_STATE_PROVIDER_TYPE: RegistryKey<
    crate::block_state_provider_type_keys::BlockStateProviderType,
> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider_type"));
impl Registered for crate::block_state_provider_type_keys::BlockStateProviderType {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER_TYPE;
}
