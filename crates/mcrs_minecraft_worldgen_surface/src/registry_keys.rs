// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const MATERIAL_CONDITION: RegistryKey<crate::proto::MaterialConditionHolder> =
    RegistryKey::new(rl!("minecraft:worldgen/material_condition"));
impl Registered for crate::proto::MaterialConditionHolder {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION;
}

pub const MATERIAL_CONDITION_TYPE: RegistryKey<
    crate::material_condition_type_keys::MaterialConditionType,
> = RegistryKey::new(rl!("minecraft:worldgen/material_condition_type"));
impl Registered for crate::material_condition_type_keys::MaterialConditionType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION_TYPE;
}

pub const MATERIAL_RULE: RegistryKey<crate::proto::MaterialRule> =
    RegistryKey::new(rl!("minecraft:worldgen/material_rule"));
impl Registered for crate::proto::MaterialRule {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE;
}

pub const MATERIAL_RULE_TYPE: RegistryKey<crate::material_rule_type_keys::MaterialRuleType> =
    RegistryKey::new(rl!("minecraft:worldgen/material_rule_type"));
impl Registered for crate::material_rule_type_keys::MaterialRuleType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[
    (
        MATERIAL_CONDITION_TYPE.location(),
        crate::material_condition_type_keys::MaterialConditionType::ENTRIES,
    ),
    (
        MATERIAL_RULE_TYPE.location(),
        crate::material_rule_type_keys::MaterialRuleType::ENTRIES,
    ),
];
