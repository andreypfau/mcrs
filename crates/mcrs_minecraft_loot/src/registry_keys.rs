// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CONTEXT_FLOAT_PROVIDER: RegistryKey<crate::loot::ContextFloatProvider> =
    RegistryKey::new(rl!("minecraft:context_float_provider"));
impl Registered for crate::loot::ContextFloatProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER;
}

pub const CONTEXT_FLOAT_PROVIDER_TYPE: RegistryKey<
    crate::context_float_provider_type_keys::ContextFloatProviderType,
> = RegistryKey::new(rl!("minecraft:context_float_provider_type"));
impl Registered for crate::context_float_provider_type_keys::ContextFloatProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER_TYPE;
}

pub const CONTEXT_INT_PROVIDER: RegistryKey<crate::loot::ContextIntProvider> =
    RegistryKey::new(rl!("minecraft:context_int_provider"));
impl Registered for crate::loot::ContextIntProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER;
}

pub const CONTEXT_INT_PROVIDER_TYPE: RegistryKey<
    crate::context_int_provider_type_keys::ContextIntProviderType,
> = RegistryKey::new(rl!("minecraft:context_int_provider_type"));
impl Registered for crate::context_int_provider_type_keys::ContextIntProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER_TYPE;
}

pub const CONTEXT_KEY_SET: RegistryKey<crate::context_key_set_keys::ContextKeySet> =
    RegistryKey::new(rl!("minecraft:context_key_set"));
impl Registered for crate::context_key_set_keys::ContextKeySet {
    const REGISTRY: RegistryKey<Self> = CONTEXT_KEY_SET;
}

pub const ITEM_MODIFIER: RegistryKey<crate::function::LootItemFunction> =
    RegistryKey::new(rl!("minecraft:item_modifier"));
impl Registered for crate::function::LootItemFunction {
    const REGISTRY: RegistryKey<Self> = ITEM_MODIFIER;
}

pub const LOOT_CONDITION_TYPE: RegistryKey<crate::loot_condition_type_keys::LootConditionType> =
    RegistryKey::new(rl!("minecraft:loot_condition_type"));
impl Registered for crate::loot_condition_type_keys::LootConditionType {
    const REGISTRY: RegistryKey<Self> = LOOT_CONDITION_TYPE;
}

pub const LOOT_FUNCTION_TYPE: RegistryKey<crate::loot_function_type_keys::LootFunctionType> =
    RegistryKey::new(rl!("minecraft:loot_function_type"));
impl Registered for crate::loot_function_type_keys::LootFunctionType {
    const REGISTRY: RegistryKey<Self> = LOOT_FUNCTION_TYPE;
}

pub const LOOT_NBT_PROVIDER_TYPE: RegistryKey<
    crate::loot_nbt_provider_type_keys::LootNbtProviderType,
> = RegistryKey::new(rl!("minecraft:loot_nbt_provider_type"));
impl Registered for crate::loot_nbt_provider_type_keys::LootNbtProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_NBT_PROVIDER_TYPE;
}

pub const LOOT_POOL_ENTRY_TYPE: RegistryKey<crate::loot_pool_entry_type_keys::LootPoolEntryType> =
    RegistryKey::new(rl!("minecraft:loot_pool_entry_type"));
impl Registered for crate::loot_pool_entry_type_keys::LootPoolEntryType {
    const REGISTRY: RegistryKey<Self> = LOOT_POOL_ENTRY_TYPE;
}

pub const LOOT_SCORE_PROVIDER_TYPE: RegistryKey<
    crate::loot_score_provider_type_keys::LootScoreProviderType,
> = RegistryKey::new(rl!("minecraft:loot_score_provider_type"));
impl Registered for crate::loot_score_provider_type_keys::LootScoreProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_SCORE_PROVIDER_TYPE;
}

pub const LOOT_TABLE: RegistryKey<crate::loot::LootTable> =
    RegistryKey::new(rl!("minecraft:loot_table"));
impl Registered for crate::loot::LootTable {
    const REGISTRY: RegistryKey<Self> = LOOT_TABLE;
}

pub const PREDICATE: RegistryKey<crate::condition::LootCondition> =
    RegistryKey::new(rl!("minecraft:predicate"));
impl Registered for crate::condition::LootCondition {
    const REGISTRY: RegistryKey<Self> = PREDICATE;
}

pub const SLOT_SOURCE: RegistryKey<crate::slot::SlotSource> =
    RegistryKey::new(rl!("minecraft:slot_source"));
impl Registered for crate::slot::SlotSource {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE;
}

pub const SLOT_SOURCE_TYPE: RegistryKey<crate::slot_source_type_keys::SlotSourceType> =
    RegistryKey::new(rl!("minecraft:slot_source_type"));
impl Registered for crate::slot_source_type_keys::SlotSourceType {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE_TYPE;
}
