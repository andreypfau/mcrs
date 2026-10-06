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
pub enum Block {}
pub const BLOCK: RegistryKey<Block> = RegistryKey::new(rl!("minecraft:block"));
impl Registered for Block {
    const REGISTRY: RegistryKey<Self> = BLOCK;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockEntityType {}
pub const BLOCK_ENTITY_TYPE: RegistryKey<BlockEntityType> = RegistryKey::new(rl!("minecraft:block_entity_type"));
impl Registered for BlockEntityType {
    const REGISTRY: RegistryKey<Self> = BLOCK_ENTITY_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandArgumentType {}
pub const COMMAND_ARGUMENT_TYPE: RegistryKey<CommandArgumentType> = RegistryKey::new(rl!("minecraft:command_argument_type"));
impl Registered for CommandArgumentType {
    const REGISTRY: RegistryKey<Self> = COMMAND_ARGUMENT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextFloatProvider {}
pub const CONTEXT_FLOAT_PROVIDER: RegistryKey<ContextFloatProvider> = RegistryKey::new(rl!("minecraft:context_float_provider"));
impl Registered for ContextFloatProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextFloatProviderType {}
pub const CONTEXT_FLOAT_PROVIDER_TYPE: RegistryKey<ContextFloatProviderType> = RegistryKey::new(rl!("minecraft:context_float_provider_type"));
impl Registered for ContextFloatProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextIntProvider {}
pub const CONTEXT_INT_PROVIDER: RegistryKey<ContextIntProvider> = RegistryKey::new(rl!("minecraft:context_int_provider"));
impl Registered for ContextIntProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextIntProviderType {}
pub const CONTEXT_INT_PROVIDER_TYPE: RegistryKey<ContextIntProviderType> = RegistryKey::new(rl!("minecraft:context_int_provider_type"));
impl Registered for ContextIntProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKeySet {}
pub const CONTEXT_KEY_SET: RegistryKey<ContextKeySet> = RegistryKey::new(rl!("minecraft:context_key_set"));
impl Registered for ContextKeySet {
    const REGISTRY: RegistryKey<Self> = CONTEXT_KEY_SET;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreativeModeTab {}
pub const CREATIVE_MODE_TAB: RegistryKey<CreativeModeTab> = RegistryKey::new(rl!("minecraft:creative_mode_tab"));
impl Registered for CreativeModeTab {
    const REGISTRY: RegistryKey<Self> = CREATIVE_MODE_TAB;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomStat {}
pub const CUSTOM_STAT: RegistryKey<CustomStat> = RegistryKey::new(rl!("minecraft:custom_stat"));
impl Registered for CustomStat {
    const REGISTRY: RegistryKey<Self> = CUSTOM_STAT;
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
pub enum EntitySubPredicateType {}
pub const ENTITY_SUB_PREDICATE_TYPE: RegistryKey<EntitySubPredicateType> = RegistryKey::new(rl!("minecraft:entity_sub_predicate_type"));
impl Registered for EntitySubPredicateType {
    const REGISTRY: RegistryKey<Self> = ENTITY_SUB_PREDICATE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatProviderType {}
pub const FLOAT_PROVIDER_TYPE: RegistryKey<FloatProviderType> = RegistryKey::new(rl!("minecraft:float_provider_type"));
impl Registered for FloatProviderType {
    const REGISTRY: RegistryKey<Self> = FLOAT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fluid {}
pub const FLUID: RegistryKey<Fluid> = RegistryKey::new(rl!("minecraft:fluid"));
impl Registered for Fluid {
    const REGISTRY: RegistryKey<Self> = FLUID;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEvent {}
pub const GAME_EVENT: RegistryKey<GameEvent> = RegistryKey::new(rl!("minecraft:game_event"));
impl Registered for GameEvent {
    const REGISTRY: RegistryKey<Self> = GAME_EVENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameRule {}
pub const GAME_RULE: RegistryKey<GameRule> = RegistryKey::new(rl!("minecraft:game_rule"));
impl Registered for GameRule {
    const REGISTRY: RegistryKey<Self> = GAME_RULE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightProviderType {}
pub const HEIGHT_PROVIDER_TYPE: RegistryKey<HeightProviderType> = RegistryKey::new(rl!("minecraft:height_provider_type"));
impl Registered for HeightProviderType {
    const REGISTRY: RegistryKey<Self> = HEIGHT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomingRpcMethods {}
pub const INCOMING_RPC_METHODS: RegistryKey<IncomingRpcMethods> = RegistryKey::new(rl!("minecraft:incoming_rpc_methods"));
impl Registered for IncomingRpcMethods {
    const REGISTRY: RegistryKey<Self> = INCOMING_RPC_METHODS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntProviderType {}
pub const INT_PROVIDER_TYPE: RegistryKey<IntProviderType> = RegistryKey::new(rl!("minecraft:int_provider_type"));
impl Registered for IntProviderType {
    const REGISTRY: RegistryKey<Self> = INT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {}
pub const ITEM: RegistryKey<Item> = RegistryKey::new(rl!("minecraft:item"));
impl Registered for Item {
    const REGISTRY: RegistryKey<Self> = ITEM;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemModifier {}
pub const ITEM_MODIFIER: RegistryKey<ItemModifier> = RegistryKey::new(rl!("minecraft:item_modifier"));
impl Registered for ItemModifier {
    const REGISTRY: RegistryKey<Self> = ITEM_MODIFIER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootFunctionType {}
pub const LOOT_FUNCTION_TYPE: RegistryKey<LootFunctionType> = RegistryKey::new(rl!("minecraft:loot_function_type"));
impl Registered for LootFunctionType {
    const REGISTRY: RegistryKey<Self> = LOOT_FUNCTION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootNbtProviderType {}
pub const LOOT_NBT_PROVIDER_TYPE: RegistryKey<LootNbtProviderType> = RegistryKey::new(rl!("minecraft:loot_nbt_provider_type"));
impl Registered for LootNbtProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_NBT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootPoolEntryType {}
pub const LOOT_POOL_ENTRY_TYPE: RegistryKey<LootPoolEntryType> = RegistryKey::new(rl!("minecraft:loot_pool_entry_type"));
impl Registered for LootPoolEntryType {
    const REGISTRY: RegistryKey<Self> = LOOT_POOL_ENTRY_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootScoreProviderType {}
pub const LOOT_SCORE_PROVIDER_TYPE: RegistryKey<LootScoreProviderType> = RegistryKey::new(rl!("minecraft:loot_score_provider_type"));
impl Registered for LootScoreProviderType {
    const REGISTRY: RegistryKey<Self> = LOOT_SCORE_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootTable {}
pub const LOOT_TABLE: RegistryKey<LootTable> = RegistryKey::new(rl!("minecraft:loot_table"));
impl Registered for LootTable {
    const REGISTRY: RegistryKey<Self> = LOOT_TABLE;
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
pub enum Predicate {}
pub const PREDICATE: RegistryKey<Predicate> = RegistryKey::new(rl!("minecraft:predicate"));
impl Registered for Predicate {
    const REGISTRY: RegistryKey<Self> = PREDICATE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recipe {}
pub const RECIPE: RegistryKey<Recipe> = RegistryKey::new(rl!("minecraft:recipe"));
impl Registered for Recipe {
    const REGISTRY: RegistryKey<Self> = RECIPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeBookCategory {}
pub const RECIPE_BOOK_CATEGORY: RegistryKey<RecipeBookCategory> = RegistryKey::new(rl!("minecraft:recipe_book_category"));
impl Registered for RecipeBookCategory {
    const REGISTRY: RegistryKey<Self> = RECIPE_BOOK_CATEGORY;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeSerializer {}
pub const RECIPE_SERIALIZER: RegistryKey<RecipeSerializer> = RegistryKey::new(rl!("minecraft:recipe_serializer"));
impl Registered for RecipeSerializer {
    const REGISTRY: RegistryKey<Self> = RECIPE_SERIALIZER;
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
pub enum SlotSource {}
pub const SLOT_SOURCE: RegistryKey<SlotSource> = RegistryKey::new(rl!("minecraft:slot_source"));
impl Registered for SlotSource {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotSourceType {}
pub const SLOT_SOURCE_TYPE: RegistryKey<SlotSourceType> = RegistryKey::new(rl!("minecraft:slot_source_type"));
impl Registered for SlotSourceType {
    const REGISTRY: RegistryKey<Self> = SLOT_SOURCE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatType {}
pub const STAT_TYPE: RegistryKey<StatType> = RegistryKey::new(rl!("minecraft:stat_type"));
impl Registered for StatType {
    const REGISTRY: RegistryKey<Self> = STAT_TYPE;
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

pub fn bindings() -> [TypeBinding; 55] {
    [
        ADVANCEMENT.binding(),
        ATTRIBUTE_TYPE.binding(),
        BLOCK.binding(),
        BLOCK_ENTITY_TYPE.binding(),
        COMMAND_ARGUMENT_TYPE.binding(),
        CONTEXT_FLOAT_PROVIDER.binding(),
        CONTEXT_FLOAT_PROVIDER_TYPE.binding(),
        CONTEXT_INT_PROVIDER.binding(),
        CONTEXT_INT_PROVIDER_TYPE.binding(),
        CONTEXT_KEY_SET.binding(),
        CREATIVE_MODE_TAB.binding(),
        CUSTOM_STAT.binding(),
        DEBUG_SUBSCRIPTION.binding(),
        DIALOG_ACTION_TYPE.binding(),
        ENCHANTMENT_PROVIDER_TYPE.binding(),
        ENTITY_SUB_PREDICATE_TYPE.binding(),
        FLOAT_PROVIDER_TYPE.binding(),
        FLUID.binding(),
        GAME_EVENT.binding(),
        GAME_RULE.binding(),
        HEIGHT_PROVIDER_TYPE.binding(),
        INCOMING_RPC_METHODS.binding(),
        INT_PROVIDER_TYPE.binding(),
        ITEM.binding(),
        ITEM_MODIFIER.binding(),
        LOOT_FUNCTION_TYPE.binding(),
        LOOT_NBT_PROVIDER_TYPE.binding(),
        LOOT_POOL_ENTRY_TYPE.binding(),
        LOOT_SCORE_PROVIDER_TYPE.binding(),
        LOOT_TABLE.binding(),
        MEMORY_MODULE_TYPE.binding(),
        NUMBER_FORMAT_TYPE.binding(),
        OUTGOING_RPC_METHODS.binding(),
        PERMISSION_CHECK_TYPE.binding(),
        PERMISSION_TYPE.binding(),
        POINT_OF_INTEREST_TYPE.binding(),
        PREDICATE.binding(),
        RECIPE.binding(),
        RECIPE_BOOK_CATEGORY.binding(),
        RECIPE_SERIALIZER.binding(),
        RECIPE_TYPE.binding(),
        SENSOR_TYPE.binding(),
        SLOT_SOURCE.binding(),
        SLOT_SOURCE_TYPE.binding(),
        STAT_TYPE.binding(),
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
