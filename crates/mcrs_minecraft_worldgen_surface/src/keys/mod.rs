// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod material_condition;
pub mod material_condition_type;
pub mod material_rule;
pub mod material_rule_type;

pub use material_condition_type::MaterialConditionType;
pub use material_rule_type::MaterialRuleType;

use mcrs_minecraft_resource::{Registered, RegistryKey, StaticResourceLocation, rl};

pub const MATERIAL_CONDITION: RegistryKey<crate::proto::MaterialConditionHolder> =
    RegistryKey::new(rl!("minecraft:worldgen/material_condition"));
impl Registered for crate::proto::MaterialConditionHolder {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION;
}

pub const MATERIAL_CONDITION_TYPE: RegistryKey<crate::keys::MaterialConditionType> =
    RegistryKey::new(rl!("minecraft:worldgen/material_condition_type"));
impl Registered for crate::keys::MaterialConditionType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION_TYPE;
}

pub const MATERIAL_RULE: RegistryKey<crate::proto::MaterialRule> =
    RegistryKey::new(rl!("minecraft:worldgen/material_rule"));
impl Registered for crate::proto::MaterialRule {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE;
}

pub const MATERIAL_RULE_TYPE: RegistryKey<crate::keys::MaterialRuleType> =
    RegistryKey::new(rl!("minecraft:worldgen/material_rule_type"));
impl Registered for crate::keys::MaterialRuleType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[
    (
        MATERIAL_CONDITION_TYPE.location(),
        MaterialConditionType::ENTRIES,
    ),
    (MATERIAL_RULE_TYPE.location(), MaterialRuleType::ENTRIES),
];
