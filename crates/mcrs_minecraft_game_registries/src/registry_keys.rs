use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const CHAT_TYPE: RegistryKey<crate::chat_type::ChatType> =
    <crate::chat_type::ChatType as Registered>::REGISTRY;
impl Registered for crate::chat_type::ChatType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:chat_type"));
}

pub const ENCHANTMENT_PROVIDER: RegistryKey<crate::enchantment_provider::EnchantmentProvider> =
    <crate::enchantment_provider::EnchantmentProvider as Registered>::REGISTRY;
impl Registered for crate::enchantment_provider::EnchantmentProvider {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:enchantment_provider"));
}

pub const SULFUR_CUBE_ARCHETYPE: RegistryKey<crate::sulfur_cube_archetype::SulfurCubeArchetype> =
    <crate::sulfur_cube_archetype::SulfurCubeArchetype as Registered>::REGISTRY;
impl Registered for crate::sulfur_cube_archetype::SulfurCubeArchetype {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:sulfur_cube_archetype"));
}

pub const TEST_ENVIRONMENT: RegistryKey<crate::test_types::TestEnvironment> =
    <crate::test_types::TestEnvironment as Registered>::REGISTRY;
impl Registered for crate::test_types::TestEnvironment {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:test_environment"));
}

pub const TEST_ENVIRONMENT_DEFINITION_TYPE: RegistryKey<crate::test_environment_definition_type_keys::TestEnvironmentDefinitionType> = <crate::test_environment_definition_type_keys::TestEnvironmentDefinitionType as Registered>::REGISTRY;
impl Registered for crate::test_environment_definition_type_keys::TestEnvironmentDefinitionType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:test_environment_definition_type"));
}

pub const TEST_INSTANCE: RegistryKey<crate::test_types::TestInstance> =
    <crate::test_types::TestInstance as Registered>::REGISTRY;
impl Registered for crate::test_types::TestInstance {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:test_instance"));
}

pub const TEST_INSTANCE_TYPE: RegistryKey<crate::test_instance_type_keys::TestInstanceType> =
    <crate::test_instance_type_keys::TestInstanceType as Registered>::REGISTRY;
impl Registered for crate::test_instance_type_keys::TestInstanceType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:test_instance_type"));
}

pub const TRADE_SET: RegistryKey<crate::villager_trade::TradeSet> =
    <crate::villager_trade::TradeSet as Registered>::REGISTRY;
impl Registered for crate::villager_trade::TradeSet {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:trade_set"));
}

pub const VILLAGER_TRADE: RegistryKey<crate::villager_trade::VillagerTrade> =
    <crate::villager_trade::VillagerTrade as Registered>::REGISTRY;
impl Registered for crate::villager_trade::VillagerTrade {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:villager_trade"));
}

pub const WORLD_PRESET: RegistryKey<crate::worldgen::world_preset::WorldPreset> =
    <crate::worldgen::world_preset::WorldPreset as Registered>::REGISTRY;
impl Registered for crate::worldgen::world_preset::WorldPreset {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/world_preset"));
}

pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[
    (
        TEST_ENVIRONMENT_DEFINITION_TYPE.location(),
        crate::test_environment_definition_type_keys::TestEnvironmentDefinitionType::ENTRIES,
    ),
    (
        TEST_INSTANCE_TYPE.location(),
        crate::test_instance_type_keys::TestInstanceType::ENTRIES,
    ),
];
