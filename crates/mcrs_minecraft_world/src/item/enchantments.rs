use std::sync::OnceLock;

use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::{RegistrySet, StaticRegistry};

use crate::registries::test_registries;

pub fn register_all_enchantments(
    registry: &mut StaticRegistry<EnchantmentData>,
    set: &RegistrySet,
) {
    let table = set
        .table("minecraft:enchantment")
        .expect("minecraft:enchantment is a loaded registry");
    let values = set
        .column::<EnchantmentData>("minecraft:enchantment")
        .expect("minecraft:enchantment holds enchantments");
    for (name, value) in table.names().iter().zip(values) {
        let leaked: &'static EnchantmentData = Box::leak(Box::new(value.clone()));
        registry.register(name.clone(), leaked);
    }
}

/// The vanilla enchantments, loaded once per process; for tests that have no
/// app to hand them a registry set from.
pub fn test_enchantments() -> &'static StaticRegistry<EnchantmentData> {
    static REGISTRY: OnceLock<StaticRegistry<EnchantmentData>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = StaticRegistry::new();
        register_all_enchantments(&mut registry, test_registries());
        registry.freeze();
        registry
    })
}
