use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_keys::Enchantment;
use mcrs_minecraft_registry::{Entries, Registry};

use crate::registries::test_registries;

pub fn test_enchantments() -> Entries<Enchantment, EnchantmentData> {
    test_registries()
        .entries()
        .expect("minecraft:enchantment holds enchantments")
}

pub fn test_enchantment_registry() -> Registry<Enchantment> {
    test_registries()
        .registry()
        .expect("minecraft:enchantment is a loaded registry")
}
