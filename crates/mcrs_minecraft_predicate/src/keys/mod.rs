// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod custom_stat;
pub mod entity_sub_predicate_type;
pub mod stat_type;

pub use custom_stat::CustomStat;
pub use entity_sub_predicate_type::EntitySubPredicateType;
pub use stat_type::StatType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const CUSTOM_STAT: RegistryKey<crate::keys::CustomStat> = RegistryKey::new(rl!("minecraft:custom_stat"));
impl Registered for crate::keys::CustomStat {
    const REGISTRY: RegistryKey<Self> = CUSTOM_STAT;
}

pub const ENTITY_SUB_PREDICATE_TYPE: RegistryKey<crate::keys::EntitySubPredicateType> = RegistryKey::new(rl!("minecraft:entity_sub_predicate_type"));
impl Registered for crate::keys::EntitySubPredicateType {
    const REGISTRY: RegistryKey<Self> = ENTITY_SUB_PREDICATE_TYPE;
}

pub const STAT_TYPE: RegistryKey<crate::keys::StatType> = RegistryKey::new(rl!("minecraft:stat_type"));
impl Registered for crate::keys::StatType {
    const REGISTRY: RegistryKey<Self> = STAT_TYPE;
}

pub fn bindings() -> [TypeBinding; 3] {
    [
        CUSTOM_STAT.binding(),
        ENTITY_SUB_PREDICATE_TYPE.binding(),
        STAT_TYPE.binding(),
    ]
}
