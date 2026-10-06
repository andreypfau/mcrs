use mcrs_minecraft_enchantment::effects::EnchantmentEffects;
use mcrs_minecraft_keys::Enchantment;
use mcrs_minecraft_registry::{Entries, Registry};

use crate::registries::test_registries;

pub fn test_enchantment_effects() -> Entries<Enchantment, Option<EnchantmentEffects>> {
    test_registries()
        .entries()
        .expect("minecraft:enchantment holds enchantment effects")
}

pub fn test_enchantment_registry() -> Registry<Enchantment> {
    test_registries()
        .registry()
        .expect("minecraft:enchantment is a loaded registry")
}
