// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Activity {}
pub const ACTIVITY: RegistryKey<Activity> = RegistryKey::new(rl!("minecraft:activity"));
impl Registered for Activity {
    const REGISTRY: RegistryKey<Self> = ACTIVITY;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Advancement {}
pub const ADVANCEMENT: RegistryKey<Advancement> = RegistryKey::new(rl!("minecraft:advancement"));
impl Registered for Advancement {
    const REGISTRY: RegistryKey<Self> = ADVANCEMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attribute {}
pub const ATTRIBUTE: RegistryKey<Attribute> = RegistryKey::new(rl!("minecraft:attribute"));
impl Registered for Attribute {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {}
pub const ATTRIBUTE_TYPE: RegistryKey<AttributeType> = RegistryKey::new(rl!("minecraft:attribute_type"));
impl Registered for AttributeType {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BannerPattern {}
pub const BANNER_PATTERN: RegistryKey<BannerPattern> = RegistryKey::new(rl!("minecraft:banner_pattern"));
impl Registered for BannerPattern {
    const REGISTRY: RegistryKey<Self> = BANNER_PATTERN;
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
pub enum BlockPredicateType {}
pub const BLOCK_PREDICATE_TYPE: RegistryKey<BlockPredicateType> = RegistryKey::new(rl!("minecraft:block_predicate_type"));
impl Registered for BlockPredicateType {
    const REGISTRY: RegistryKey<Self> = BLOCK_PREDICATE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockTransformer {}
pub const BLOCK_TRANSFORMER: RegistryKey<BlockTransformer> = RegistryKey::new(rl!("minecraft:block_transformer"));
impl Registered for BlockTransformer {
    const REGISTRY: RegistryKey<Self> = BLOCK_TRANSFORMER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatSoundVariant {}
pub const CAT_SOUND_VARIANT: RegistryKey<CatSoundVariant> = RegistryKey::new(rl!("minecraft:cat_sound_variant"));
impl Registered for CatSoundVariant {
    const REGISTRY: RegistryKey<Self> = CAT_SOUND_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatVariant {}
pub const CAT_VARIANT: RegistryKey<CatVariant> = RegistryKey::new(rl!("minecraft:cat_variant"));
impl Registered for CatVariant {
    const REGISTRY: RegistryKey<Self> = CAT_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatType {}
pub const CHAT_TYPE: RegistryKey<ChatType> = RegistryKey::new(rl!("minecraft:chat_type"));
impl Registered for ChatType {
    const REGISTRY: RegistryKey<Self> = CHAT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenSoundVariant {}
pub const CHICKEN_SOUND_VARIANT: RegistryKey<ChickenSoundVariant> = RegistryKey::new(rl!("minecraft:chicken_sound_variant"));
impl Registered for ChickenSoundVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_SOUND_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenVariant {}
pub const CHICKEN_VARIANT: RegistryKey<ChickenVariant> = RegistryKey::new(rl!("minecraft:chicken_variant"));
impl Registered for ChickenVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkStatus {}
pub const CHUNK_STATUS: RegistryKey<ChunkStatus> = RegistryKey::new(rl!("minecraft:chunk_status"));
impl Registered for ChunkStatus {
    const REGISTRY: RegistryKey<Self> = CHUNK_STATUS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandArgumentType {}
pub const COMMAND_ARGUMENT_TYPE: RegistryKey<CommandArgumentType> = RegistryKey::new(rl!("minecraft:command_argument_type"));
impl Registered for CommandArgumentType {
    const REGISTRY: RegistryKey<Self> = COMMAND_ARGUMENT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsumeEffectType {}
pub const CONSUME_EFFECT_TYPE: RegistryKey<ConsumeEffectType> = RegistryKey::new(rl!("minecraft:consume_effect_type"));
impl Registered for ConsumeEffectType {
    const REGISTRY: RegistryKey<Self> = CONSUME_EFFECT_TYPE;
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
pub enum CowSoundVariant {}
pub const COW_SOUND_VARIANT: RegistryKey<CowSoundVariant> = RegistryKey::new(rl!("minecraft:cow_sound_variant"));
impl Registered for CowSoundVariant {
    const REGISTRY: RegistryKey<Self> = COW_SOUND_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CowVariant {}
pub const COW_VARIANT: RegistryKey<CowVariant> = RegistryKey::new(rl!("minecraft:cow_variant"));
impl Registered for CowVariant {
    const REGISTRY: RegistryKey<Self> = COW_VARIANT;
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
pub enum DamageType {}
pub const DAMAGE_TYPE: RegistryKey<DamageType> = RegistryKey::new(rl!("minecraft:damage_type"));
impl Registered for DamageType {
    const REGISTRY: RegistryKey<Self> = DAMAGE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataComponentPredicateType {}
pub const DATA_COMPONENT_PREDICATE_TYPE: RegistryKey<DataComponentPredicateType> = RegistryKey::new(rl!("minecraft:data_component_predicate_type"));
impl Registered for DataComponentPredicateType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_PREDICATE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataComponentType {}
pub const DATA_COMPONENT_TYPE: RegistryKey<DataComponentType> = RegistryKey::new(rl!("minecraft:data_component_type"));
impl Registered for DataComponentType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugSubscription {}
pub const DEBUG_SUBSCRIPTION: RegistryKey<DebugSubscription> = RegistryKey::new(rl!("minecraft:debug_subscription"));
impl Registered for DebugSubscription {
    const REGISTRY: RegistryKey<Self> = DEBUG_SUBSCRIPTION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecoratedPotPattern {}
pub const DECORATED_POT_PATTERN: RegistryKey<DecoratedPotPattern> = RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
impl Registered for DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = DECORATED_POT_PATTERN;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialog {}
pub const DIALOG: RegistryKey<Dialog> = RegistryKey::new(rl!("minecraft:dialog"));
impl Registered for Dialog {
    const REGISTRY: RegistryKey<Self> = DIALOG;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogActionType {}
pub const DIALOG_ACTION_TYPE: RegistryKey<DialogActionType> = RegistryKey::new(rl!("minecraft:dialog_action_type"));
impl Registered for DialogActionType {
    const REGISTRY: RegistryKey<Self> = DIALOG_ACTION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogBodyType {}
pub const DIALOG_BODY_TYPE: RegistryKey<DialogBodyType> = RegistryKey::new(rl!("minecraft:dialog_body_type"));
impl Registered for DialogBodyType {
    const REGISTRY: RegistryKey<Self> = DIALOG_BODY_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogType {}
pub const DIALOG_TYPE: RegistryKey<DialogType> = RegistryKey::new(rl!("minecraft:dialog_type"));
impl Registered for DialogType {
    const REGISTRY: RegistryKey<Self> = DIALOG_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {}
pub const DIMENSION: RegistryKey<Dimension> = RegistryKey::new(rl!("minecraft:dimension"));
impl Registered for Dimension {
    const REGISTRY: RegistryKey<Self> = DIMENSION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionType {}
pub const DIMENSION_TYPE: RegistryKey<DimensionType> = RegistryKey::new(rl!("minecraft:dimension_type"));
impl Registered for DimensionType {
    const REGISTRY: RegistryKey<Self> = DIMENSION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Enchantment {}
pub const ENCHANTMENT: RegistryKey<Enchantment> = RegistryKey::new(rl!("minecraft:enchantment"));
impl Registered for Enchantment {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentEffectComponentType {}
pub const ENCHANTMENT_EFFECT_COMPONENT_TYPE: RegistryKey<EnchantmentEffectComponentType> = RegistryKey::new(rl!("minecraft:enchantment_effect_component_type"));
impl Registered for EnchantmentEffectComponentType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_EFFECT_COMPONENT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentEntityEffectType {}
pub const ENCHANTMENT_ENTITY_EFFECT_TYPE: RegistryKey<EnchantmentEntityEffectType> = RegistryKey::new(rl!("minecraft:enchantment_entity_effect_type"));
impl Registered for EnchantmentEntityEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_ENTITY_EFFECT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentLevelBasedValueType {}
pub const ENCHANTMENT_LEVEL_BASED_VALUE_TYPE: RegistryKey<EnchantmentLevelBasedValueType> = RegistryKey::new(rl!("minecraft:enchantment_level_based_value_type"));
impl Registered for EnchantmentLevelBasedValueType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LEVEL_BASED_VALUE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentLocationBasedEffectType {}
pub const ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE: RegistryKey<EnchantmentLocationBasedEffectType> = RegistryKey::new(rl!("minecraft:enchantment_location_based_effect_type"));
impl Registered for EnchantmentLocationBasedEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentProvider {}
pub const ENCHANTMENT_PROVIDER: RegistryKey<EnchantmentProvider> = RegistryKey::new(rl!("minecraft:enchantment_provider"));
impl Registered for EnchantmentProvider {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_PROVIDER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentProviderType {}
pub const ENCHANTMENT_PROVIDER_TYPE: RegistryKey<EnchantmentProviderType> = RegistryKey::new(rl!("minecraft:enchantment_provider_type"));
impl Registered for EnchantmentProviderType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentValueEffectType {}
pub const ENCHANTMENT_VALUE_EFFECT_TYPE: RegistryKey<EnchantmentValueEffectType> = RegistryKey::new(rl!("minecraft:enchantment_value_effect_type"));
impl Registered for EnchantmentValueEffectType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_VALUE_EFFECT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntitySubPredicateType {}
pub const ENTITY_SUB_PREDICATE_TYPE: RegistryKey<EntitySubPredicateType> = RegistryKey::new(rl!("minecraft:entity_sub_predicate_type"));
impl Registered for EntitySubPredicateType {
    const REGISTRY: RegistryKey<Self> = ENTITY_SUB_PREDICATE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {}
pub const ENTITY_TYPE: RegistryKey<EntityType> = RegistryKey::new(rl!("minecraft:entity_type"));
impl Registered for EntityType {
    const REGISTRY: RegistryKey<Self> = ENTITY_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvironmentAttribute {}
pub const ENVIRONMENT_ATTRIBUTE: RegistryKey<EnvironmentAttribute> = RegistryKey::new(rl!("minecraft:environment_attribute"));
impl Registered for EnvironmentAttribute {
    const REGISTRY: RegistryKey<Self> = ENVIRONMENT_ATTRIBUTE;
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
pub enum FrogVariant {}
pub const FROG_VARIANT: RegistryKey<FrogVariant> = RegistryKey::new(rl!("minecraft:frog_variant"));
impl Registered for FrogVariant {
    const REGISTRY: RegistryKey<Self> = FROG_VARIANT;
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
pub enum InputControlType {}
pub const INPUT_CONTROL_TYPE: RegistryKey<InputControlType> = RegistryKey::new(rl!("minecraft:input_control_type"));
impl Registered for InputControlType {
    const REGISTRY: RegistryKey<Self> = INPUT_CONTROL_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instrument {}
pub const INSTRUMENT: RegistryKey<Instrument> = RegistryKey::new(rl!("minecraft:instrument"));
impl Registered for Instrument {
    const REGISTRY: RegistryKey<Self> = INSTRUMENT;
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
pub enum JukeboxSong {}
pub const JUKEBOX_SONG: RegistryKey<JukeboxSong> = RegistryKey::new(rl!("minecraft:jukebox_song"));
impl Registered for JukeboxSong {
    const REGISTRY: RegistryKey<Self> = JUKEBOX_SONG;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootConditionType {}
pub const LOOT_CONDITION_TYPE: RegistryKey<LootConditionType> = RegistryKey::new(rl!("minecraft:loot_condition_type"));
impl Registered for LootConditionType {
    const REGISTRY: RegistryKey<Self> = LOOT_CONDITION_TYPE;
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
pub enum MapDecorationType {}
pub const MAP_DECORATION_TYPE: RegistryKey<MapDecorationType> = RegistryKey::new(rl!("minecraft:map_decoration_type"));
impl Registered for MapDecorationType {
    const REGISTRY: RegistryKey<Self> = MAP_DECORATION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryModuleType {}
pub const MEMORY_MODULE_TYPE: RegistryKey<MemoryModuleType> = RegistryKey::new(rl!("minecraft:memory_module_type"));
impl Registered for MemoryModuleType {
    const REGISTRY: RegistryKey<Self> = MEMORY_MODULE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Menu {}
pub const MENU: RegistryKey<Menu> = RegistryKey::new(rl!("minecraft:menu"));
impl Registered for Menu {
    const REGISTRY: RegistryKey<Self> = MENU;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobEffect {}
pub const MOB_EFFECT: RegistryKey<MobEffect> = RegistryKey::new(rl!("minecraft:mob_effect"));
impl Registered for MobEffect {
    const REGISTRY: RegistryKey<Self> = MOB_EFFECT;
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
pub enum PaintingVariant {}
pub const PAINTING_VARIANT: RegistryKey<PaintingVariant> = RegistryKey::new(rl!("minecraft:painting_variant"));
impl Registered for PaintingVariant {
    const REGISTRY: RegistryKey<Self> = PAINTING_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticleType {}
pub const PARTICLE_TYPE: RegistryKey<ParticleType> = RegistryKey::new(rl!("minecraft:particle_type"));
impl Registered for ParticleType {
    const REGISTRY: RegistryKey<Self> = PARTICLE_TYPE;
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
pub enum PigSoundVariant {}
pub const PIG_SOUND_VARIANT: RegistryKey<PigSoundVariant> = RegistryKey::new(rl!("minecraft:pig_sound_variant"));
impl Registered for PigSoundVariant {
    const REGISTRY: RegistryKey<Self> = PIG_SOUND_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PigVariant {}
pub const PIG_VARIANT: RegistryKey<PigVariant> = RegistryKey::new(rl!("minecraft:pig_variant"));
impl Registered for PigVariant {
    const REGISTRY: RegistryKey<Self> = PIG_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointOfInterestType {}
pub const POINT_OF_INTEREST_TYPE: RegistryKey<PointOfInterestType> = RegistryKey::new(rl!("minecraft:point_of_interest_type"));
impl Registered for PointOfInterestType {
    const REGISTRY: RegistryKey<Self> = POINT_OF_INTEREST_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PosRuleTest {}
pub const POS_RULE_TEST: RegistryKey<PosRuleTest> = RegistryKey::new(rl!("minecraft:pos_rule_test"));
impl Registered for PosRuleTest {
    const REGISTRY: RegistryKey<Self> = POS_RULE_TEST;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionSourceType {}
pub const POSITION_SOURCE_TYPE: RegistryKey<PositionSourceType> = RegistryKey::new(rl!("minecraft:position_source_type"));
impl Registered for PositionSourceType {
    const REGISTRY: RegistryKey<Self> = POSITION_SOURCE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Potion {}
pub const POTION: RegistryKey<Potion> = RegistryKey::new(rl!("minecraft:potion"));
impl Registered for Potion {
    const REGISTRY: RegistryKey<Self> = POTION;
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
pub enum RecipeDisplay {}
pub const RECIPE_DISPLAY: RegistryKey<RecipeDisplay> = RegistryKey::new(rl!("minecraft:recipe_display"));
impl Registered for RecipeDisplay {
    const REGISTRY: RegistryKey<Self> = RECIPE_DISPLAY;
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
pub enum RuleBlockEntityModifier {}
pub const RULE_BLOCK_ENTITY_MODIFIER: RegistryKey<RuleBlockEntityModifier> = RegistryKey::new(rl!("minecraft:rule_block_entity_modifier"));
impl Registered for RuleBlockEntityModifier {
    const REGISTRY: RegistryKey<Self> = RULE_BLOCK_ENTITY_MODIFIER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleTestType {}
pub const RULE_TEST_TYPE: RegistryKey<RuleTestType> = RegistryKey::new(rl!("minecraft:rule_test_type"));
impl Registered for RuleTestType {
    const REGISTRY: RegistryKey<Self> = RULE_TEST_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorType {}
pub const SENSOR_TYPE: RegistryKey<SensorType> = RegistryKey::new(rl!("minecraft:sensor_type"));
impl Registered for SensorType {
    const REGISTRY: RegistryKey<Self> = SENSOR_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotDisplay {}
pub const SLOT_DISPLAY: RegistryKey<SlotDisplay> = RegistryKey::new(rl!("minecraft:slot_display"));
impl Registered for SlotDisplay {
    const REGISTRY: RegistryKey<Self> = SLOT_DISPLAY;
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
pub enum SoundEvent {}
pub const SOUND_EVENT: RegistryKey<SoundEvent> = RegistryKey::new(rl!("minecraft:sound_event"));
impl Registered for SoundEvent {
    const REGISTRY: RegistryKey<Self> = SOUND_EVENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnConditionType {}
pub const SPAWN_CONDITION_TYPE: RegistryKey<SpawnConditionType> = RegistryKey::new(rl!("minecraft:spawn_condition_type"));
impl Registered for SpawnConditionType {
    const REGISTRY: RegistryKey<Self> = SPAWN_CONDITION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatType {}
pub const STAT_TYPE: RegistryKey<StatType> = RegistryKey::new(rl!("minecraft:stat_type"));
impl Registered for StatType {
    const REGISTRY: RegistryKey<Self> = STAT_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SulfurCubeArchetype {}
pub const SULFUR_CUBE_ARCHETYPE: RegistryKey<SulfurCubeArchetype> = RegistryKey::new(rl!("minecraft:sulfur_cube_archetype"));
impl Registered for SulfurCubeArchetype {
    const REGISTRY: RegistryKey<Self> = SULFUR_CUBE_ARCHETYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestEnvironment {}
pub const TEST_ENVIRONMENT: RegistryKey<TestEnvironment> = RegistryKey::new(rl!("minecraft:test_environment"));
impl Registered for TestEnvironment {
    const REGISTRY: RegistryKey<Self> = TEST_ENVIRONMENT;
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
pub enum TestInstance {}
pub const TEST_INSTANCE: RegistryKey<TestInstance> = RegistryKey::new(rl!("minecraft:test_instance"));
impl Registered for TestInstance {
    const REGISTRY: RegistryKey<Self> = TEST_INSTANCE;
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
pub enum Timeline {}
pub const TIMELINE: RegistryKey<Timeline> = RegistryKey::new(rl!("minecraft:timeline"));
impl Registered for Timeline {
    const REGISTRY: RegistryKey<Self> = TIMELINE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeSet {}
pub const TRADE_SET: RegistryKey<TradeSet> = RegistryKey::new(rl!("minecraft:trade_set"));
impl Registered for TradeSet {
    const REGISTRY: RegistryKey<Self> = TRADE_SET;
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
pub enum TrimMaterial {}
pub const TRIM_MATERIAL: RegistryKey<TrimMaterial> = RegistryKey::new(rl!("minecraft:trim_material"));
impl Registered for TrimMaterial {
    const REGISTRY: RegistryKey<Self> = TRIM_MATERIAL;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrimPattern {}
pub const TRIM_PATTERN: RegistryKey<TrimPattern> = RegistryKey::new(rl!("minecraft:trim_pattern"));
impl Registered for TrimPattern {
    const REGISTRY: RegistryKey<Self> = TRIM_PATTERN;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerProfession {}
pub const VILLAGER_PROFESSION: RegistryKey<VillagerProfession> = RegistryKey::new(rl!("minecraft:villager_profession"));
impl Registered for VillagerProfession {
    const REGISTRY: RegistryKey<Self> = VILLAGER_PROFESSION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerTrade {}
pub const VILLAGER_TRADE: RegistryKey<VillagerTrade> = RegistryKey::new(rl!("minecraft:villager_trade"));
impl Registered for VillagerTrade {
    const REGISTRY: RegistryKey<Self> = VILLAGER_TRADE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerType {}
pub const VILLAGER_TYPE: RegistryKey<VillagerType> = RegistryKey::new(rl!("minecraft:villager_type"));
impl Registered for VillagerType {
    const REGISTRY: RegistryKey<Self> = VILLAGER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfSoundVariant {}
pub const WOLF_SOUND_VARIANT: RegistryKey<WolfSoundVariant> = RegistryKey::new(rl!("minecraft:wolf_sound_variant"));
impl Registered for WolfSoundVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_SOUND_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfVariant {}
pub const WOLF_VARIANT: RegistryKey<WolfVariant> = RegistryKey::new(rl!("minecraft:wolf_variant"));
impl Registered for WolfVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_VARIANT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldClock {}
pub const WORLD_CLOCK: RegistryKey<WorldClock> = RegistryKey::new(rl!("minecraft:world_clock"));
impl Registered for WorldClock {
    const REGISTRY: RegistryKey<Self> = WORLD_CLOCK;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {}
pub const BIOME: RegistryKey<Biome> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
impl Registered for Biome {
    const REGISTRY: RegistryKey<Self> = BIOME;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeSource {}
pub const BIOME_SOURCE: RegistryKey<BiomeSource> = RegistryKey::new(rl!("minecraft:worldgen/biome_source"));
impl Registered for BiomeSource {
    const REGISTRY: RegistryKey<Self> = BIOME_SOURCE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockStateProvider {}
pub const BLOCK_STATE_PROVIDER: RegistryKey<BlockStateProvider> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider"));
impl Registered for BlockStateProvider {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockStateProviderType {}
pub const BLOCK_STATE_PROVIDER_TYPE: RegistryKey<BlockStateProviderType> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider_type"));
impl Registered for BlockStateProviderType {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Carver {}
pub const CARVER: RegistryKey<Carver> = RegistryKey::new(rl!("minecraft:worldgen/carver"));
impl Registered for Carver {
    const REGISTRY: RegistryKey<Self> = CARVER;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarverType {}
pub const CARVER_TYPE: RegistryKey<CarverType> = RegistryKey::new(rl!("minecraft:worldgen/carver_type"));
impl Registered for CarverType {
    const REGISTRY: RegistryKey<Self> = CARVER_TYPE;
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
pub enum DensityFunctionType {}
pub const DENSITY_FUNCTION_TYPE: RegistryKey<DensityFunctionType> = RegistryKey::new(rl!("minecraft:worldgen/density_function_type"));
impl Registered for DensityFunctionType {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {}
pub const FEATURE: RegistryKey<Feature> = RegistryKey::new(rl!("minecraft:worldgen/feature"));
impl Registered for Feature {
    const REGISTRY: RegistryKey<Self> = FEATURE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureSizeType {}
pub const FEATURE_SIZE_TYPE: RegistryKey<FeatureSizeType> = RegistryKey::new(rl!("minecraft:worldgen/feature_size_type"));
impl Registered for FeatureSizeType {
    const REGISTRY: RegistryKey<Self> = FEATURE_SIZE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureType {}
pub const FEATURE_TYPE: RegistryKey<FeatureType> = RegistryKey::new(rl!("minecraft:worldgen/feature_type"));
impl Registered for FeatureType {
    const REGISTRY: RegistryKey<Self> = FEATURE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlatLevelGeneratorPreset {}
pub const FLAT_LEVEL_GENERATOR_PRESET: RegistryKey<FlatLevelGeneratorPreset> = RegistryKey::new(rl!("minecraft:worldgen/flat_level_generator_preset"));
impl Registered for FlatLevelGeneratorPreset {
    const REGISTRY: RegistryKey<Self> = FLAT_LEVEL_GENERATOR_PRESET;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoliagePlacerType {}
pub const FOLIAGE_PLACER_TYPE: RegistryKey<FoliagePlacerType> = RegistryKey::new(rl!("minecraft:worldgen/foliage_placer_type"));
impl Registered for FoliagePlacerType {
    const REGISTRY: RegistryKey<Self> = FOLIAGE_PLACER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialCondition {}
pub const MATERIAL_CONDITION: RegistryKey<MaterialCondition> = RegistryKey::new(rl!("minecraft:worldgen/material_condition"));
impl Registered for MaterialCondition {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialConditionType {}
pub const MATERIAL_CONDITION_TYPE: RegistryKey<MaterialConditionType> = RegistryKey::new(rl!("minecraft:worldgen/material_condition_type"));
impl Registered for MaterialConditionType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_CONDITION_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialRule {}
pub const MATERIAL_RULE: RegistryKey<MaterialRule> = RegistryKey::new(rl!("minecraft:worldgen/material_rule"));
impl Registered for MaterialRule {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialRuleType {}
pub const MATERIAL_RULE_TYPE: RegistryKey<MaterialRuleType> = RegistryKey::new(rl!("minecraft:worldgen/material_rule_type"));
impl Registered for MaterialRuleType {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiNoiseBiomeSourceParameterList {}
pub const MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST: RegistryKey<MultiNoiseBiomeSourceParameterList> = RegistryKey::new(rl!("minecraft:worldgen/multi_noise_biome_source_parameter_list"));
impl Registered for MultiNoiseBiomeSourceParameterList {
    const REGISTRY: RegistryKey<Self> = MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Noise {}
pub const NOISE: RegistryKey<Noise> = RegistryKey::new(rl!("minecraft:worldgen/noise"));
impl Registered for Noise {
    const REGISTRY: RegistryKey<Self> = NOISE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseSettings {}
pub const NOISE_SETTINGS: RegistryKey<NoiseSettings> = RegistryKey::new(rl!("minecraft:worldgen/noise_settings"));
impl Registered for NoiseSettings {
    const REGISTRY: RegistryKey<Self> = NOISE_SETTINGS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacedFeature {}
pub const PLACED_FEATURE: RegistryKey<PlacedFeature> = RegistryKey::new(rl!("minecraft:worldgen/placed_feature"));
impl Registered for PlacedFeature {
    const REGISTRY: RegistryKey<Self> = PLACED_FEATURE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacementModifierType {}
pub const PLACEMENT_MODIFIER_TYPE: RegistryKey<PlacementModifierType> = RegistryKey::new(rl!("minecraft:worldgen/placement_modifier_type"));
impl Registered for PlacementModifierType {
    const REGISTRY: RegistryKey<Self> = PLACEMENT_MODIFIER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolAliasBinding {}
pub const POOL_ALIAS_BINDING: RegistryKey<PoolAliasBinding> = RegistryKey::new(rl!("minecraft:worldgen/pool_alias_binding"));
impl Registered for PoolAliasBinding {
    const REGISTRY: RegistryKey<Self> = POOL_ALIAS_BINDING;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessorList {}
pub const PROCESSOR_LIST: RegistryKey<ProcessorList> = RegistryKey::new(rl!("minecraft:worldgen/processor_list"));
impl Registered for ProcessorList {
    const REGISTRY: RegistryKey<Self> = PROCESSOR_LIST;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootPlacerType {}
pub const ROOT_PLACER_TYPE: RegistryKey<RootPlacerType> = RegistryKey::new(rl!("minecraft:worldgen/root_placer_type"));
impl Registered for RootPlacerType {
    const REGISTRY: RegistryKey<Self> = ROOT_PLACER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Structure {}
pub const STRUCTURE: RegistryKey<Structure> = RegistryKey::new(rl!("minecraft:worldgen/structure"));
impl Registered for Structure {
    const REGISTRY: RegistryKey<Self> = STRUCTURE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePiece {}
pub const STRUCTURE_PIECE: RegistryKey<StructurePiece> = RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
impl Registered for StructurePiece {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PIECE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePlacement {}
pub const STRUCTURE_PLACEMENT: RegistryKey<StructurePlacement> = RegistryKey::new(rl!("minecraft:worldgen/structure_placement"));
impl Registered for StructurePlacement {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PLACEMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePoolElement {}
pub const STRUCTURE_POOL_ELEMENT: RegistryKey<StructurePoolElement> = RegistryKey::new(rl!("minecraft:worldgen/structure_pool_element"));
impl Registered for StructurePoolElement {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_POOL_ELEMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureProcessor {}
pub const STRUCTURE_PROCESSOR: RegistryKey<StructureProcessor> = RegistryKey::new(rl!("minecraft:worldgen/structure_processor"));
impl Registered for StructureProcessor {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PROCESSOR;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureSet {}
pub const STRUCTURE_SET: RegistryKey<StructureSet> = RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
impl Registered for StructureSet {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_SET;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureType {}
pub const STRUCTURE_TYPE: RegistryKey<StructureType> = RegistryKey::new(rl!("minecraft:worldgen/structure_type"));
impl Registered for StructureType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplatePool {}
pub const TEMPLATE_POOL: RegistryKey<TemplatePool> = RegistryKey::new(rl!("minecraft:worldgen/template_pool"));
impl Registered for TemplatePool {
    const REGISTRY: RegistryKey<Self> = TEMPLATE_POOL;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeDecoratorType {}
pub const TREE_DECORATOR_TYPE: RegistryKey<TreeDecoratorType> = RegistryKey::new(rl!("minecraft:worldgen/tree_decorator_type"));
impl Registered for TreeDecoratorType {
    const REGISTRY: RegistryKey<Self> = TREE_DECORATOR_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrunkPlacerType {}
pub const TRUNK_PLACER_TYPE: RegistryKey<TrunkPlacerType> = RegistryKey::new(rl!("minecraft:worldgen/trunk_placer_type"));
impl Registered for TrunkPlacerType {
    const REGISTRY: RegistryKey<Self> = TRUNK_PLACER_TYPE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldPreset {}
pub const WORLD_PRESET: RegistryKey<WorldPreset> = RegistryKey::new(rl!("minecraft:worldgen/world_preset"));
impl Registered for WorldPreset {
    const REGISTRY: RegistryKey<Self> = WORLD_PRESET;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZombieNautilusVariant {}
pub const ZOMBIE_NAUTILUS_VARIANT: RegistryKey<ZombieNautilusVariant> = RegistryKey::new(rl!("minecraft:zombie_nautilus_variant"));
impl Registered for ZombieNautilusVariant {
    const REGISTRY: RegistryKey<Self> = ZOMBIE_NAUTILUS_VARIANT;
}

pub fn bindings() -> [TypeBinding; 155] {
    [
        ACTIVITY.binding(),
        ADVANCEMENT.binding(),
        ATTRIBUTE.binding(),
        ATTRIBUTE_TYPE.binding(),
        BANNER_PATTERN.binding(),
        BLOCK.binding(),
        BLOCK_ENTITY_TYPE.binding(),
        BLOCK_PREDICATE_TYPE.binding(),
        BLOCK_TRANSFORMER.binding(),
        CAT_SOUND_VARIANT.binding(),
        CAT_VARIANT.binding(),
        CHAT_TYPE.binding(),
        CHICKEN_SOUND_VARIANT.binding(),
        CHICKEN_VARIANT.binding(),
        CHUNK_STATUS.binding(),
        COMMAND_ARGUMENT_TYPE.binding(),
        CONSUME_EFFECT_TYPE.binding(),
        CONTEXT_FLOAT_PROVIDER.binding(),
        CONTEXT_FLOAT_PROVIDER_TYPE.binding(),
        CONTEXT_INT_PROVIDER.binding(),
        CONTEXT_INT_PROVIDER_TYPE.binding(),
        CONTEXT_KEY_SET.binding(),
        COW_SOUND_VARIANT.binding(),
        COW_VARIANT.binding(),
        CREATIVE_MODE_TAB.binding(),
        CUSTOM_STAT.binding(),
        DAMAGE_TYPE.binding(),
        DATA_COMPONENT_PREDICATE_TYPE.binding(),
        DATA_COMPONENT_TYPE.binding(),
        DEBUG_SUBSCRIPTION.binding(),
        DECORATED_POT_PATTERN.binding(),
        DIALOG.binding(),
        DIALOG_ACTION_TYPE.binding(),
        DIALOG_BODY_TYPE.binding(),
        DIALOG_TYPE.binding(),
        DIMENSION.binding(),
        DIMENSION_TYPE.binding(),
        ENCHANTMENT.binding(),
        ENCHANTMENT_EFFECT_COMPONENT_TYPE.binding(),
        ENCHANTMENT_ENTITY_EFFECT_TYPE.binding(),
        ENCHANTMENT_LEVEL_BASED_VALUE_TYPE.binding(),
        ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE.binding(),
        ENCHANTMENT_PROVIDER.binding(),
        ENCHANTMENT_PROVIDER_TYPE.binding(),
        ENCHANTMENT_VALUE_EFFECT_TYPE.binding(),
        ENTITY_SUB_PREDICATE_TYPE.binding(),
        ENTITY_TYPE.binding(),
        ENVIRONMENT_ATTRIBUTE.binding(),
        FLOAT_PROVIDER_TYPE.binding(),
        FLUID.binding(),
        FROG_VARIANT.binding(),
        GAME_EVENT.binding(),
        GAME_RULE.binding(),
        HEIGHT_PROVIDER_TYPE.binding(),
        INCOMING_RPC_METHODS.binding(),
        INPUT_CONTROL_TYPE.binding(),
        INSTRUMENT.binding(),
        INT_PROVIDER_TYPE.binding(),
        ITEM.binding(),
        ITEM_MODIFIER.binding(),
        JUKEBOX_SONG.binding(),
        LOOT_CONDITION_TYPE.binding(),
        LOOT_FUNCTION_TYPE.binding(),
        LOOT_NBT_PROVIDER_TYPE.binding(),
        LOOT_POOL_ENTRY_TYPE.binding(),
        LOOT_SCORE_PROVIDER_TYPE.binding(),
        LOOT_TABLE.binding(),
        MAP_DECORATION_TYPE.binding(),
        MEMORY_MODULE_TYPE.binding(),
        MENU.binding(),
        MOB_EFFECT.binding(),
        NUMBER_FORMAT_TYPE.binding(),
        OUTGOING_RPC_METHODS.binding(),
        PAINTING_VARIANT.binding(),
        PARTICLE_TYPE.binding(),
        PERMISSION_CHECK_TYPE.binding(),
        PERMISSION_TYPE.binding(),
        PIG_SOUND_VARIANT.binding(),
        PIG_VARIANT.binding(),
        POINT_OF_INTEREST_TYPE.binding(),
        POS_RULE_TEST.binding(),
        POSITION_SOURCE_TYPE.binding(),
        POTION.binding(),
        PREDICATE.binding(),
        RECIPE.binding(),
        RECIPE_BOOK_CATEGORY.binding(),
        RECIPE_DISPLAY.binding(),
        RECIPE_SERIALIZER.binding(),
        RECIPE_TYPE.binding(),
        RULE_BLOCK_ENTITY_MODIFIER.binding(),
        RULE_TEST_TYPE.binding(),
        SENSOR_TYPE.binding(),
        SLOT_DISPLAY.binding(),
        SLOT_SOURCE.binding(),
        SLOT_SOURCE_TYPE.binding(),
        SOUND_EVENT.binding(),
        SPAWN_CONDITION_TYPE.binding(),
        STAT_TYPE.binding(),
        SULFUR_CUBE_ARCHETYPE.binding(),
        TEST_ENVIRONMENT.binding(),
        TEST_ENVIRONMENT_DEFINITION_TYPE.binding(),
        TEST_FUNCTION.binding(),
        TEST_INSTANCE.binding(),
        TEST_INSTANCE_TYPE.binding(),
        TICKET_TYPE.binding(),
        TIMELINE.binding(),
        TRADE_SET.binding(),
        TRIAL_SPAWNER.binding(),
        TRIGGER_TYPE.binding(),
        TRIM_MATERIAL.binding(),
        TRIM_PATTERN.binding(),
        VILLAGER_PROFESSION.binding(),
        VILLAGER_TRADE.binding(),
        VILLAGER_TYPE.binding(),
        WOLF_SOUND_VARIANT.binding(),
        WOLF_VARIANT.binding(),
        WORLD_CLOCK.binding(),
        BIOME.binding(),
        BIOME_SOURCE.binding(),
        BLOCK_STATE_PROVIDER.binding(),
        BLOCK_STATE_PROVIDER_TYPE.binding(),
        CARVER.binding(),
        CARVER_TYPE.binding(),
        CHUNK_GENERATOR.binding(),
        DENSITY_FUNCTION.binding(),
        DENSITY_FUNCTION_TYPE.binding(),
        FEATURE.binding(),
        FEATURE_SIZE_TYPE.binding(),
        FEATURE_TYPE.binding(),
        FLAT_LEVEL_GENERATOR_PRESET.binding(),
        FOLIAGE_PLACER_TYPE.binding(),
        MATERIAL_CONDITION.binding(),
        MATERIAL_CONDITION_TYPE.binding(),
        MATERIAL_RULE.binding(),
        MATERIAL_RULE_TYPE.binding(),
        MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST.binding(),
        NOISE.binding(),
        NOISE_SETTINGS.binding(),
        PLACED_FEATURE.binding(),
        PLACEMENT_MODIFIER_TYPE.binding(),
        POOL_ALIAS_BINDING.binding(),
        PROCESSOR_LIST.binding(),
        ROOT_PLACER_TYPE.binding(),
        STRUCTURE.binding(),
        STRUCTURE_PIECE.binding(),
        STRUCTURE_PLACEMENT.binding(),
        STRUCTURE_POOL_ELEMENT.binding(),
        STRUCTURE_PROCESSOR.binding(),
        STRUCTURE_SET.binding(),
        STRUCTURE_TYPE.binding(),
        TEMPLATE_POOL.binding(),
        TREE_DECORATOR_TYPE.binding(),
        TRUNK_PLACER_TYPE.binding(),
        WORLD_PRESET.binding(),
        ZOMBIE_NAUTILUS_VARIANT.binding(),
    ]
}
