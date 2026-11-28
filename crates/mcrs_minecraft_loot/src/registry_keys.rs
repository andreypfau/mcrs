use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CONTEXT_FLOAT_PROVIDER: RegistryKey<crate::loot::ContextFloatProvider> =
    <crate::loot::ContextFloatProvider as Registered>::REGISTRY;
impl Registered for crate::loot::ContextFloatProvider {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:context_float_provider"));
}

pub const CONTEXT_FLOAT_PROVIDER_TYPE: RegistryKey<
    crate::context_float_provider_type_keys::ContextFloatProviderType,
> = <crate::context_float_provider_type_keys::ContextFloatProviderType as Registered>::REGISTRY;
impl Registered for crate::context_float_provider_type_keys::ContextFloatProviderType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:context_float_provider_type"));
}

pub const CONTEXT_INT_PROVIDER: RegistryKey<crate::loot::ContextIntProvider> =
    <crate::loot::ContextIntProvider as Registered>::REGISTRY;
impl Registered for crate::loot::ContextIntProvider {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:context_int_provider"));
}

pub const CONTEXT_INT_PROVIDER_TYPE: RegistryKey<
    crate::context_int_provider_type_keys::ContextIntProviderType,
> = <crate::context_int_provider_type_keys::ContextIntProviderType as Registered>::REGISTRY;
impl Registered for crate::context_int_provider_type_keys::ContextIntProviderType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:context_int_provider_type"));
}

pub const CONTEXT_KEY_SET: RegistryKey<crate::context_key_set_keys::ContextKeySet> =
    <crate::context_key_set_keys::ContextKeySet as Registered>::REGISTRY;
impl Registered for crate::context_key_set_keys::ContextKeySet {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:context_key_set"));
}

pub const ITEM_MODIFIER: RegistryKey<crate::function::LootItemFunction> =
    <crate::function::LootItemFunction as Registered>::REGISTRY;
impl Registered for crate::function::LootItemFunction {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:item_modifier"));
}

pub const LOOT_CONDITION_TYPE: RegistryKey<crate::loot_condition_type_keys::LootConditionType> =
    <crate::loot_condition_type_keys::LootConditionType as Registered>::REGISTRY;
impl Registered for crate::loot_condition_type_keys::LootConditionType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_condition_type"));
}

pub const LOOT_FUNCTION_TYPE: RegistryKey<crate::loot_function_type_keys::LootFunctionType> =
    <crate::loot_function_type_keys::LootFunctionType as Registered>::REGISTRY;
impl Registered for crate::loot_function_type_keys::LootFunctionType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_function_type"));
}

pub const LOOT_NBT_PROVIDER_TYPE: RegistryKey<
    crate::loot_nbt_provider_type_keys::LootNbtProviderType,
> = <crate::loot_nbt_provider_type_keys::LootNbtProviderType as Registered>::REGISTRY;
impl Registered for crate::loot_nbt_provider_type_keys::LootNbtProviderType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_nbt_provider_type"));
}

pub const LOOT_POOL_ENTRY_TYPE: RegistryKey<crate::loot_pool_entry_type_keys::LootPoolEntryType> =
    <crate::loot_pool_entry_type_keys::LootPoolEntryType as Registered>::REGISTRY;
impl Registered for crate::loot_pool_entry_type_keys::LootPoolEntryType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_pool_entry_type"));
}

pub const LOOT_SCORE_PROVIDER_TYPE: RegistryKey<
    crate::loot_score_provider_type_keys::LootScoreProviderType,
> = <crate::loot_score_provider_type_keys::LootScoreProviderType as Registered>::REGISTRY;
impl Registered for crate::loot_score_provider_type_keys::LootScoreProviderType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_score_provider_type"));
}

pub const LOOT_TABLE: RegistryKey<crate::loot::LootTable> =
    <crate::loot::LootTable as Registered>::REGISTRY;
impl Registered for crate::loot::LootTable {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:loot_table"));
}

pub const PREDICATE: RegistryKey<crate::condition::LootCondition> =
    <crate::condition::LootCondition as Registered>::REGISTRY;
impl Registered for crate::condition::LootCondition {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:predicate"));
}

pub const SLOT_SOURCE: RegistryKey<crate::slot::SlotSource> =
    <crate::slot::SlotSource as Registered>::REGISTRY;
impl Registered for crate::slot::SlotSource {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:slot_source"));
}

pub const SLOT_SOURCE_TYPE: RegistryKey<crate::slot_source_type_keys::SlotSourceType> =
    <crate::slot_source_type_keys::SlotSourceType as Registered>::REGISTRY;
impl Registered for crate::slot_source_type_keys::SlotSourceType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:slot_source_type"));
}
