// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod chat_type;
pub mod enchantment_provider;
pub mod sulfur_cube_archetype;
pub mod test_environment;
pub mod test_instance;
pub mod trade_set;
pub mod villager_trade;
pub mod villager_trade_tags;
pub mod world_preset;
pub mod world_preset_tags;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const CHAT_TYPE: RegistryKey<crate::chat_type::ChatType> = RegistryKey::new(rl!("minecraft:chat_type"));
impl Registered for crate::chat_type::ChatType {
    const REGISTRY: RegistryKey<Self> = CHAT_TYPE;
}

pub const ENCHANTMENT_PROVIDER: RegistryKey<crate::enchantment_provider::EnchantmentProvider> = RegistryKey::new(rl!("minecraft:enchantment_provider"));
impl Registered for crate::enchantment_provider::EnchantmentProvider {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_PROVIDER;
}

pub const SULFUR_CUBE_ARCHETYPE: RegistryKey<crate::sulfur_cube_archetype::SulfurCubeArchetype> = RegistryKey::new(rl!("minecraft:sulfur_cube_archetype"));
impl Registered for crate::sulfur_cube_archetype::SulfurCubeArchetype {
    const REGISTRY: RegistryKey<Self> = SULFUR_CUBE_ARCHETYPE;
}

pub const TEST_ENVIRONMENT: RegistryKey<crate::test_types::TestEnvironment> = RegistryKey::new(rl!("minecraft:test_environment"));
impl Registered for crate::test_types::TestEnvironment {
    const REGISTRY: RegistryKey<Self> = TEST_ENVIRONMENT;
}

pub const TEST_INSTANCE: RegistryKey<crate::test_types::TestInstance> = RegistryKey::new(rl!("minecraft:test_instance"));
impl Registered for crate::test_types::TestInstance {
    const REGISTRY: RegistryKey<Self> = TEST_INSTANCE;
}

pub const TRADE_SET: RegistryKey<crate::villager_trade::TradeSet> = RegistryKey::new(rl!("minecraft:trade_set"));
impl Registered for crate::villager_trade::TradeSet {
    const REGISTRY: RegistryKey<Self> = TRADE_SET;
}

pub const VILLAGER_TRADE: RegistryKey<crate::villager_trade::VillagerTrade> = RegistryKey::new(rl!("minecraft:villager_trade"));
impl Registered for crate::villager_trade::VillagerTrade {
    const REGISTRY: RegistryKey<Self> = VILLAGER_TRADE;
}

pub const WORLD_PRESET: RegistryKey<crate::worldgen::world_preset::WorldPreset> = RegistryKey::new(rl!("minecraft:worldgen/world_preset"));
impl Registered for crate::worldgen::world_preset::WorldPreset {
    const REGISTRY: RegistryKey<Self> = WORLD_PRESET;
}

pub fn bindings() -> [TypeBinding; 8] {
    [
        CHAT_TYPE.binding(),
        ENCHANTMENT_PROVIDER.binding(),
        SULFUR_CUBE_ARCHETYPE.binding(),
        TEST_ENVIRONMENT.binding(),
        TEST_INSTANCE.binding(),
        TRADE_SET.binding(),
        VILLAGER_TRADE.binding(),
        WORLD_PRESET.binding(),
    ]
}
