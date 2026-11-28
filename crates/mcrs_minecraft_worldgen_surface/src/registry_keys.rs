use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const MATERIAL_CONDITION: RegistryKey<crate::proto::MaterialConditionHolder> =
    <crate::proto::MaterialConditionHolder as Registered>::REGISTRY;
impl Registered for crate::proto::MaterialConditionHolder {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/material_condition"));
}

pub const MATERIAL_CONDITION_TYPE: RegistryKey<
    crate::material_condition_type_keys::MaterialConditionType,
> = <crate::material_condition_type_keys::MaterialConditionType as Registered>::REGISTRY;
impl Registered for crate::material_condition_type_keys::MaterialConditionType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/material_condition_type"));
}

pub const MATERIAL_RULE: RegistryKey<crate::proto::MaterialRule> =
    <crate::proto::MaterialRule as Registered>::REGISTRY;
impl Registered for crate::proto::MaterialRule {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/material_rule"));
}

pub const MATERIAL_RULE_TYPE: RegistryKey<crate::material_rule_type_keys::MaterialRuleType> =
    <crate::material_rule_type_keys::MaterialRuleType as Registered>::REGISTRY;
impl Registered for crate::material_rule_type_keys::MaterialRuleType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/material_rule_type"));
}

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
