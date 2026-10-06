// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod loot_condition_type;
pub mod loot_function_type;
pub mod loot_nbt_provider_type;
pub mod loot_pool_entry_type;
pub mod loot_score_provider_type;
pub mod predicate;
pub mod slot_source_type;

pub use loot_condition_type::LootConditionType;
pub use loot_function_type::LootFunctionType;
pub use loot_nbt_provider_type::LootNbtProviderType;
pub use loot_pool_entry_type::LootPoolEntryType;
pub use loot_score_provider_type::LootScoreProviderType;
pub use slot_source_type::SlotSourceType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const ITEM_MODIFIER: RegistryKey<crate::function::LootItemFunction> = RegistryKey::new(rl!("minecraft:item_modifier"));
impl Registered for crate::function::LootItemFunction {
    const REGISTRY: RegistryKey<Self> = ITEM_MODIFIER;
}

pub const LOOT_CONDITION_TYPE: RegistryKey<crate::keys::LootConditionType> = RegistryKey::new(rl!("minecraft:loot_condition_type"));
impl Registered for crate::keys::LootConditionType {
    const REGISTRY: RegistryKey<Self> = LOOT_CONDITION_TYPE;
}

pub const LOOT_FUNCTION_TYPE: RegistryKey<crate::keys::LootFunctionType> = RegistryKey::new(rl!("minecraft:loot_function_type"));
impl Registered for crate::keys::LootFunctionType {
    const REGISTRY: RegistryKey<Self> = LOOT_FUNCTION_TYPE;
}

pub const LOOT_NBT_PROVIDER_TYPE: RegistryKey<crate::keys::LootNbtProviderType> = RegistryKey::new(rl!("minecraft:loot_nbt_provider_type"));
impl Registered for crate::keys::LootNbtProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_NBT_PROVIDER_TYPE;
}

pub const LOOT_POOL_ENTRY_TYPE: RegistryKey<crate::keys::LootPoolEntryType> = RegistryKey::new(rl!("minecraft:loot_pool_entry_type"));
impl Registered for crate::keys::LootPoolEntryType {
    const REGISTRY: RegistryKey<Self> = LOOT_POOL_ENTRY_TYPE;
}

pub const LOOT_SCORE_PROVIDER_TYPE: RegistryKey<crate::keys::LootScoreProviderType> = RegistryKey::new(rl!("minecraft:loot_score_provider_type"));
impl Registered for crate::keys::LootScoreProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_SCORE_PROVIDER_TYPE;
}

pub const PREDICATE: RegistryKey<crate::condition::LootCondition> = RegistryKey::new(rl!("minecraft:predicate"));
impl Registered for crate::condition::LootCondition {
    const REGISTRY: RegistryKey<Self> = PREDICATE;
}

pub const SLOT_SOURCE: RegistryKey<crate::slot::SlotSource> = RegistryKey::new(rl!("minecraft:slot_source"));
impl Registered for crate::slot::SlotSource {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE;
}

pub const SLOT_SOURCE_TYPE: RegistryKey<crate::keys::SlotSourceType> = RegistryKey::new(rl!("minecraft:slot_source_type"));
impl Registered for crate::keys::SlotSourceType {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE_TYPE;
}

pub fn bindings() -> [TypeBinding; 9] {
    [
        ITEM_MODIFIER.binding(),
        LOOT_CONDITION_TYPE.binding(),
        LOOT_FUNCTION_TYPE.binding(),
        LOOT_NBT_PROVIDER_TYPE.binding(),
        LOOT_POOL_ENTRY_TYPE.binding(),
        LOOT_SCORE_PROVIDER_TYPE.binding(),
        PREDICATE.binding(),
        SLOT_SOURCE.binding(),
        SLOT_SOURCE_TYPE.binding(),
    ]
}
