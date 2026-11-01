// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CUSTOM_STAT: RegistryKey<crate::custom_stat_keys::CustomStat> =
    RegistryKey::new(rl!("minecraft:custom_stat"));
impl Registered for crate::custom_stat_keys::CustomStat {
    const REGISTRY: RegistryKey<Self> = CUSTOM_STAT;
}

pub const ENTITY_SUB_PREDICATE_TYPE: RegistryKey<
    crate::entity_sub_predicate_type_keys::EntitySubPredicateType,
> = RegistryKey::new(rl!("minecraft:entity_sub_predicate_type"));
impl Registered for crate::entity_sub_predicate_type_keys::EntitySubPredicateType {
    const REGISTRY: RegistryKey<Self> = ENTITY_SUB_PREDICATE_TYPE;
}

pub const STAT_TYPE: RegistryKey<crate::stat_type_keys::StatType> =
    RegistryKey::new(rl!("minecraft:stat_type"));
impl Registered for crate::stat_type_keys::StatType {
    const REGISTRY: RegistryKey<Self> = STAT_TYPE;
}
