use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_PREDICATE_TYPE: RegistryKey<crate::block_predicate_type_keys::BlockPredicateType> =
    <crate::block_predicate_type_keys::BlockPredicateType as Registered>::REGISTRY;
impl Registered for crate::block_predicate_type_keys::BlockPredicateType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:block_predicate_type"));
}

pub const BLOCK_STATE_PROVIDER: RegistryKey<crate::provider::DirectBlockStateProvider> =
    <crate::provider::DirectBlockStateProvider as Registered>::REGISTRY;
impl Registered for crate::provider::DirectBlockStateProvider {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/block_state_provider"));
}

pub const BLOCK_STATE_PROVIDER_TYPE: RegistryKey<
    crate::block_state_provider_type_keys::BlockStateProviderType,
> = <crate::block_state_provider_type_keys::BlockStateProviderType as Registered>::REGISTRY;
impl Registered for crate::block_state_provider_type_keys::BlockStateProviderType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/block_state_provider_type"));
}
