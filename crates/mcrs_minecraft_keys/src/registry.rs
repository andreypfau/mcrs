// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Advancement {}
pub const ADVANCEMENT: RegistryKey<Advancement> = RegistryKey::new(rl!("minecraft:advancement"));
impl Registered for Advancement {
    const REGISTRY: RegistryKey<Self> = ADVANCEMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {}
pub const ATTRIBUTE_TYPE: RegistryKey<AttributeType> = RegistryKey::new(rl!("minecraft:attribute_type"));
impl Registered for AttributeType {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandArgumentType {}
pub const COMMAND_ARGUMENT_TYPE: RegistryKey<CommandArgumentType> = RegistryKey::new(rl!("minecraft:command_argument_type"));
impl Registered for CommandArgumentType {
    const REGISTRY: RegistryKey<Self> = COMMAND_ARGUMENT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreativeModeTab {}
pub const CREATIVE_MODE_TAB: RegistryKey<CreativeModeTab> = RegistryKey::new(rl!("minecraft:creative_mode_tab"));
impl Registered for CreativeModeTab {
    const REGISTRY: RegistryKey<Self> = CREATIVE_MODE_TAB;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugSubscription {}
pub const DEBUG_SUBSCRIPTION: RegistryKey<DebugSubscription> = RegistryKey::new(rl!("minecraft:debug_subscription"));
impl Registered for DebugSubscription {
    const REGISTRY: RegistryKey<Self> = DEBUG_SUBSCRIPTION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogActionType {}
pub const DIALOG_ACTION_TYPE: RegistryKey<DialogActionType> = RegistryKey::new(rl!("minecraft:dialog_action_type"));
impl Registered for DialogActionType {
    const REGISTRY: RegistryKey<Self> = DIALOG_ACTION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentProviderType {}
pub const ENCHANTMENT_PROVIDER_TYPE: RegistryKey<EnchantmentProviderType> = RegistryKey::new(rl!("minecraft:enchantment_provider_type"));
impl Registered for EnchantmentProviderType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEvent {}
pub const GAME_EVENT: RegistryKey<GameEvent> = RegistryKey::new(rl!("minecraft:game_event"));
impl Registered for GameEvent {
    const REGISTRY: RegistryKey<Self> = GAME_EVENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomingRpcMethods {}
pub const INCOMING_RPC_METHODS: RegistryKey<IncomingRpcMethods> = RegistryKey::new(rl!("minecraft:incoming_rpc_methods"));
impl Registered for IncomingRpcMethods {
    const REGISTRY: RegistryKey<Self> = INCOMING_RPC_METHODS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryModuleType {}
pub const MEMORY_MODULE_TYPE: RegistryKey<MemoryModuleType> = RegistryKey::new(rl!("minecraft:memory_module_type"));
impl Registered for MemoryModuleType {
    const REGISTRY: RegistryKey<Self> = MEMORY_MODULE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberFormatType {}
pub const NUMBER_FORMAT_TYPE: RegistryKey<NumberFormatType> = RegistryKey::new(rl!("minecraft:number_format_type"));
impl Registered for NumberFormatType {
    const REGISTRY: RegistryKey<Self> = NUMBER_FORMAT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutgoingRpcMethods {}
pub const OUTGOING_RPC_METHODS: RegistryKey<OutgoingRpcMethods> = RegistryKey::new(rl!("minecraft:outgoing_rpc_methods"));
impl Registered for OutgoingRpcMethods {
    const REGISTRY: RegistryKey<Self> = OUTGOING_RPC_METHODS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionCheckType {}
pub const PERMISSION_CHECK_TYPE: RegistryKey<PermissionCheckType> = RegistryKey::new(rl!("minecraft:permission_check_type"));
impl Registered for PermissionCheckType {
    const REGISTRY: RegistryKey<Self> = PERMISSION_CHECK_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {}
pub const PERMISSION_TYPE: RegistryKey<PermissionType> = RegistryKey::new(rl!("minecraft:permission_type"));
impl Registered for PermissionType {
    const REGISTRY: RegistryKey<Self> = PERMISSION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointOfInterestType {}
pub const POINT_OF_INTEREST_TYPE: RegistryKey<PointOfInterestType> = RegistryKey::new(rl!("minecraft:point_of_interest_type"));
impl Registered for PointOfInterestType {
    const REGISTRY: RegistryKey<Self> = POINT_OF_INTEREST_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeType {}
pub const RECIPE_TYPE: RegistryKey<RecipeType> = RegistryKey::new(rl!("minecraft:recipe_type"));
impl Registered for RecipeType {
    const REGISTRY: RegistryKey<Self> = RECIPE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorType {}
pub const SENSOR_TYPE: RegistryKey<SensorType> = RegistryKey::new(rl!("minecraft:sensor_type"));
impl Registered for SensorType {
    const REGISTRY: RegistryKey<Self> = SENSOR_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestEnvironmentDefinitionType {}
pub const TEST_ENVIRONMENT_DEFINITION_TYPE: RegistryKey<TestEnvironmentDefinitionType> = RegistryKey::new(rl!("minecraft:test_environment_definition_type"));
impl Registered for TestEnvironmentDefinitionType {
    const REGISTRY: RegistryKey<Self> = TEST_ENVIRONMENT_DEFINITION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestFunction {}
pub const TEST_FUNCTION: RegistryKey<TestFunction> = RegistryKey::new(rl!("minecraft:test_function"));
impl Registered for TestFunction {
    const REGISTRY: RegistryKey<Self> = TEST_FUNCTION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestInstanceType {}
pub const TEST_INSTANCE_TYPE: RegistryKey<TestInstanceType> = RegistryKey::new(rl!("minecraft:test_instance_type"));
impl Registered for TestInstanceType {
    const REGISTRY: RegistryKey<Self> = TEST_INSTANCE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TicketType {}
pub const TICKET_TYPE: RegistryKey<TicketType> = RegistryKey::new(rl!("minecraft:ticket_type"));
impl Registered for TicketType {
    const REGISTRY: RegistryKey<Self> = TICKET_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrialSpawner {}
pub const TRIAL_SPAWNER: RegistryKey<TrialSpawner> = RegistryKey::new(rl!("minecraft:trial_spawner"));
impl Registered for TrialSpawner {
    const REGISTRY: RegistryKey<Self> = TRIAL_SPAWNER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {}
pub const TRIGGER_TYPE: RegistryKey<TriggerType> = RegistryKey::new(rl!("minecraft:trigger_type"));
impl Registered for TriggerType {
    const REGISTRY: RegistryKey<Self> = TRIGGER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkGenerator {}
pub const CHUNK_GENERATOR: RegistryKey<ChunkGenerator> = RegistryKey::new(rl!("minecraft:worldgen/chunk_generator"));
impl Registered for ChunkGenerator {
    const REGISTRY: RegistryKey<Self> = CHUNK_GENERATOR;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DensityFunction {}
pub const DENSITY_FUNCTION: RegistryKey<DensityFunction> = RegistryKey::new(rl!("minecraft:worldgen/density_function"));
impl Registered for DensityFunction {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlatLevelGeneratorPreset {}
pub const FLAT_LEVEL_GENERATOR_PRESET: RegistryKey<FlatLevelGeneratorPreset> = RegistryKey::new(rl!("minecraft:worldgen/flat_level_generator_preset"));
impl Registered for FlatLevelGeneratorPreset {
    const REGISTRY: RegistryKey<Self> = FLAT_LEVEL_GENERATOR_PRESET;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialCondition {}
pub const MATERIAL_CONDITION: RegistryKey<MaterialCondition> = RegistryKey::new(rl!("minecraft:worldgen/material_condition"));
impl Registered for MaterialCondition {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION;
}

pub fn bindings() -> [TypeBinding; 27] {
    [
        ADVANCEMENT.binding(),
        ATTRIBUTE_TYPE.binding(),
        COMMAND_ARGUMENT_TYPE.binding(),
        CREATIVE_MODE_TAB.binding(),
        DEBUG_SUBSCRIPTION.binding(),
        DIALOG_ACTION_TYPE.binding(),
        ENCHANTMENT_PROVIDER_TYPE.binding(),
        GAME_EVENT.binding(),
        INCOMING_RPC_METHODS.binding(),
        MEMORY_MODULE_TYPE.binding(),
        NUMBER_FORMAT_TYPE.binding(),
        OUTGOING_RPC_METHODS.binding(),
        PERMISSION_CHECK_TYPE.binding(),
        PERMISSION_TYPE.binding(),
        POINT_OF_INTEREST_TYPE.binding(),
        RECIPE_TYPE.binding(),
        SENSOR_TYPE.binding(),
        TEST_ENVIRONMENT_DEFINITION_TYPE.binding(),
        TEST_FUNCTION.binding(),
        TEST_INSTANCE_TYPE.binding(),
        TICKET_TYPE.binding(),
        TRIAL_SPAWNER.binding(),
        TRIGGER_TYPE.binding(),
        CHUNK_GENERATOR.binding(),
        DENSITY_FUNCTION.binding(),
        FLAT_LEVEL_GENERATOR_PRESET.binding(),
        MATERIAL_CONDITION.binding(),
    ]
}
