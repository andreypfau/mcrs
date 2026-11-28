use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CUSTOM_STAT: RegistryKey<crate::custom_stat_keys::CustomStat> =
    <crate::custom_stat_keys::CustomStat as Registered>::REGISTRY;
impl Registered for crate::custom_stat_keys::CustomStat {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:custom_stat"));
}

pub const ENTITY_SUB_PREDICATE_TYPE: RegistryKey<
    crate::entity_sub_predicate_type_keys::EntitySubPredicateType,
> = <crate::entity_sub_predicate_type_keys::EntitySubPredicateType as Registered>::REGISTRY;
impl Registered for crate::entity_sub_predicate_type_keys::EntitySubPredicateType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:entity_sub_predicate_type"));
}

pub const STAT_TYPE: RegistryKey<crate::stat_type_keys::StatType> =
    <crate::stat_type_keys::StatType as Registered>::REGISTRY;
impl Registered for crate::stat_type_keys::StatType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:stat_type"));
}
