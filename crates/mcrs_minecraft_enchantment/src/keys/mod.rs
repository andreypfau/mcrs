// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod enchantment_effect_component_type;
pub mod enchantment_entity_effect_type;
pub mod enchantment_level_based_value_type;
pub mod enchantment_location_based_effect_type;
pub mod enchantment_value_effect_type;
pub mod loot_condition_type;

pub use enchantment_effect_component_type::EnchantmentEffectComponentType;
pub use enchantment_entity_effect_type::EnchantmentEntityEffectType;
pub use enchantment_level_based_value_type::EnchantmentLevelBasedValueType;
pub use enchantment_location_based_effect_type::EnchantmentLocationBasedEffectType;
pub use enchantment_value_effect_type::EnchantmentValueEffectType;
pub use loot_condition_type::LootConditionType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const ENCHANTMENT_EFFECT_COMPONENT_TYPE: RegistryKey<crate::keys::EnchantmentEffectComponentType> = RegistryKey::new(rl!("minecraft:enchantment_effect_component_type"));
impl Registered for crate::keys::EnchantmentEffectComponentType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_EFFECT_COMPONENT_TYPE;
}

pub const ENCHANTMENT_ENTITY_EFFECT_TYPE: RegistryKey<crate::keys::EnchantmentEntityEffectType> = RegistryKey::new(rl!("minecraft:enchantment_entity_effect_type"));
impl Registered for crate::keys::EnchantmentEntityEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_ENTITY_EFFECT_TYPE;
}

pub const ENCHANTMENT_LEVEL_BASED_VALUE_TYPE: RegistryKey<crate::keys::EnchantmentLevelBasedValueType> = RegistryKey::new(rl!("minecraft:enchantment_level_based_value_type"));
impl Registered for crate::keys::EnchantmentLevelBasedValueType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LEVEL_BASED_VALUE_TYPE;
}

pub const ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE: RegistryKey<crate::keys::EnchantmentLocationBasedEffectType> = RegistryKey::new(rl!("minecraft:enchantment_location_based_effect_type"));
impl Registered for crate::keys::EnchantmentLocationBasedEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE;
}

pub const ENCHANTMENT_VALUE_EFFECT_TYPE: RegistryKey<crate::keys::EnchantmentValueEffectType> = RegistryKey::new(rl!("minecraft:enchantment_value_effect_type"));
impl Registered for crate::keys::EnchantmentValueEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_VALUE_EFFECT_TYPE;
}

pub const LOOT_CONDITION_TYPE: RegistryKey<crate::keys::LootConditionType> = RegistryKey::new(rl!("minecraft:loot_condition_type"));
impl Registered for crate::keys::LootConditionType {
    const REGISTRY: RegistryKey<Self> = LOOT_CONDITION_TYPE;
}

pub fn bindings() -> [TypeBinding; 6] {
    [
        ENCHANTMENT_EFFECT_COMPONENT_TYPE.binding(),
        ENCHANTMENT_ENTITY_EFFECT_TYPE.binding(),
        ENCHANTMENT_LEVEL_BASED_VALUE_TYPE.binding(),
        ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE.binding(),
        ENCHANTMENT_VALUE_EFFECT_TYPE.binding(),
        LOOT_CONDITION_TYPE.binding(),
    ]
}
