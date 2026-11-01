// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ENCHANTMENT_EFFECT_COMPONENT_TYPE: RegistryKey<
    crate::enchantment_effect_component_type_keys::EnchantmentEffectComponentType,
> = RegistryKey::new(rl!("minecraft:enchantment_effect_component_type"));
impl Registered for crate::enchantment_effect_component_type_keys::EnchantmentEffectComponentType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_EFFECT_COMPONENT_TYPE;
}

pub const ENCHANTMENT_ENTITY_EFFECT_TYPE: RegistryKey<
    crate::enchantment_entity_effect_type_keys::EnchantmentEntityEffectType,
> = RegistryKey::new(rl!("minecraft:enchantment_entity_effect_type"));
impl Registered for crate::enchantment_entity_effect_type_keys::EnchantmentEntityEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_ENTITY_EFFECT_TYPE;
}

pub const ENCHANTMENT_LEVEL_BASED_VALUE_TYPE: RegistryKey<
    crate::enchantment_level_based_value_type_keys::EnchantmentLevelBasedValueType,
> = RegistryKey::new(rl!("minecraft:enchantment_level_based_value_type"));
impl Registered for crate::enchantment_level_based_value_type_keys::EnchantmentLevelBasedValueType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LEVEL_BASED_VALUE_TYPE;
}

pub const ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE: RegistryKey<
    crate::enchantment_location_based_effect_type_keys::EnchantmentLocationBasedEffectType,
> = RegistryKey::new(rl!("minecraft:enchantment_location_based_effect_type"));
impl Registered
    for crate::enchantment_location_based_effect_type_keys::EnchantmentLocationBasedEffectType
{
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE;
}

pub const ENCHANTMENT_PROVIDER_TYPE: RegistryKey<
    crate::enchantment_provider_type_keys::EnchantmentProviderType,
> = RegistryKey::new(rl!("minecraft:enchantment_provider_type"));
impl Registered for crate::enchantment_provider_type_keys::EnchantmentProviderType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_PROVIDER_TYPE;
}

pub const ENCHANTMENT_VALUE_EFFECT_TYPE: RegistryKey<
    crate::enchantment_value_effect_type_keys::EnchantmentValueEffectType,
> = RegistryKey::new(rl!("minecraft:enchantment_value_effect_type"));
impl Registered for crate::enchantment_value_effect_type_keys::EnchantmentValueEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_VALUE_EFFECT_TYPE;
}
