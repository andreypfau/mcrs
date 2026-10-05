// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{RegistryKey, StaticResourceLocation, rl};
use mcrs_minecraft_registry::StaticRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Activity {}
impl RegistryKey for Activity {
    const KEY: StaticResourceLocation = rl!("minecraft:activity");
}
impl StaticRegistry for Activity {
    const NAMES: &'static [&'static str] = crate::activity::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Advancement {}
impl RegistryKey for Advancement {
    const KEY: StaticResourceLocation = rl!("minecraft:advancement");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attribute {}
impl RegistryKey for Attribute {
    const KEY: StaticResourceLocation = rl!("minecraft:attribute");
}
impl StaticRegistry for Attribute {
    const NAMES: &'static [&'static str] = crate::attribute::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {}
impl RegistryKey for AttributeType {
    const KEY: StaticResourceLocation = rl!("minecraft:attribute_type");
}
impl StaticRegistry for AttributeType {
    const NAMES: &'static [&'static str] = crate::attribute_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BannerPattern {}
impl RegistryKey for BannerPattern {
    const KEY: StaticResourceLocation = rl!("minecraft:banner_pattern");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {}
impl RegistryKey for Block {
    const KEY: StaticResourceLocation = rl!("minecraft:block");
}
impl StaticRegistry for Block {
    const NAMES: &'static [&'static str] = crate::block::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockEntityType {}
impl RegistryKey for BlockEntityType {
    const KEY: StaticResourceLocation = rl!("minecraft:block_entity_type");
}
impl StaticRegistry for BlockEntityType {
    const NAMES: &'static [&'static str] = crate::block_entity_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockPredicateType {}
impl RegistryKey for BlockPredicateType {
    const KEY: StaticResourceLocation = rl!("minecraft:block_predicate_type");
}
impl StaticRegistry for BlockPredicateType {
    const NAMES: &'static [&'static str] = crate::block_predicate_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockTransformer {}
impl RegistryKey for BlockTransformer {
    const KEY: StaticResourceLocation = rl!("minecraft:block_transformer");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatSoundVariant {}
impl RegistryKey for CatSoundVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:cat_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatVariant {}
impl RegistryKey for CatVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:cat_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatType {}
impl RegistryKey for ChatType {
    const KEY: StaticResourceLocation = rl!("minecraft:chat_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenSoundVariant {}
impl RegistryKey for ChickenSoundVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:chicken_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenVariant {}
impl RegistryKey for ChickenVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:chicken_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkStatus {}
impl RegistryKey for ChunkStatus {
    const KEY: StaticResourceLocation = rl!("minecraft:chunk_status");
}
impl StaticRegistry for ChunkStatus {
    const NAMES: &'static [&'static str] = crate::chunk_status::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandArgumentType {}
impl RegistryKey for CommandArgumentType {
    const KEY: StaticResourceLocation = rl!("minecraft:command_argument_type");
}
impl StaticRegistry for CommandArgumentType {
    const NAMES: &'static [&'static str] = crate::command_argument_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsumeEffectType {}
impl RegistryKey for ConsumeEffectType {
    const KEY: StaticResourceLocation = rl!("minecraft:consume_effect_type");
}
impl StaticRegistry for ConsumeEffectType {
    const NAMES: &'static [&'static str] = crate::consume_effect_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextFloatProvider {}
impl RegistryKey for ContextFloatProvider {
    const KEY: StaticResourceLocation = rl!("minecraft:context_float_provider");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextFloatProviderType {}
impl RegistryKey for ContextFloatProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:context_float_provider_type");
}
impl StaticRegistry for ContextFloatProviderType {
    const NAMES: &'static [&'static str] = crate::context_float_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextIntProvider {}
impl RegistryKey for ContextIntProvider {
    const KEY: StaticResourceLocation = rl!("minecraft:context_int_provider");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextIntProviderType {}
impl RegistryKey for ContextIntProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:context_int_provider_type");
}
impl StaticRegistry for ContextIntProviderType {
    const NAMES: &'static [&'static str] = crate::context_int_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKeySet {}
impl RegistryKey for ContextKeySet {
    const KEY: StaticResourceLocation = rl!("minecraft:context_key_set");
}
impl StaticRegistry for ContextKeySet {
    const NAMES: &'static [&'static str] = crate::context_key_set::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CowSoundVariant {}
impl RegistryKey for CowSoundVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:cow_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CowVariant {}
impl RegistryKey for CowVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:cow_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreativeModeTab {}
impl RegistryKey for CreativeModeTab {
    const KEY: StaticResourceLocation = rl!("minecraft:creative_mode_tab");
}
impl StaticRegistry for CreativeModeTab {
    const NAMES: &'static [&'static str] = crate::creative_mode_tab::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomStat {}
impl RegistryKey for CustomStat {
    const KEY: StaticResourceLocation = rl!("minecraft:custom_stat");
}
impl StaticRegistry for CustomStat {
    const NAMES: &'static [&'static str] = crate::custom_stat::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {}
impl RegistryKey for DamageType {
    const KEY: StaticResourceLocation = rl!("minecraft:damage_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataComponentPredicateType {}
impl RegistryKey for DataComponentPredicateType {
    const KEY: StaticResourceLocation = rl!("minecraft:data_component_predicate_type");
}
impl StaticRegistry for DataComponentPredicateType {
    const NAMES: &'static [&'static str] = crate::data_component_predicate_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataComponentType {}
impl RegistryKey for DataComponentType {
    const KEY: StaticResourceLocation = rl!("minecraft:data_component_type");
}
impl StaticRegistry for DataComponentType {
    const NAMES: &'static [&'static str] = crate::data_component_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugSubscription {}
impl RegistryKey for DebugSubscription {
    const KEY: StaticResourceLocation = rl!("minecraft:debug_subscription");
}
impl StaticRegistry for DebugSubscription {
    const NAMES: &'static [&'static str] = crate::debug_subscription::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecoratedPotPattern {}
impl RegistryKey for DecoratedPotPattern {
    const KEY: StaticResourceLocation = rl!("minecraft:decorated_pot_pattern");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialog {}
impl RegistryKey for Dialog {
    const KEY: StaticResourceLocation = rl!("minecraft:dialog");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogActionType {}
impl RegistryKey for DialogActionType {
    const KEY: StaticResourceLocation = rl!("minecraft:dialog_action_type");
}
impl StaticRegistry for DialogActionType {
    const NAMES: &'static [&'static str] = crate::dialog_action_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogBodyType {}
impl RegistryKey for DialogBodyType {
    const KEY: StaticResourceLocation = rl!("minecraft:dialog_body_type");
}
impl StaticRegistry for DialogBodyType {
    const NAMES: &'static [&'static str] = crate::dialog_body_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogType {}
impl RegistryKey for DialogType {
    const KEY: StaticResourceLocation = rl!("minecraft:dialog_type");
}
impl StaticRegistry for DialogType {
    const NAMES: &'static [&'static str] = crate::dialog_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {}
impl RegistryKey for Dimension {
    const KEY: StaticResourceLocation = rl!("minecraft:dimension");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionType {}
impl RegistryKey for DimensionType {
    const KEY: StaticResourceLocation = rl!("minecraft:dimension_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Enchantment {}
impl RegistryKey for Enchantment {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentEffectComponentType {}
impl RegistryKey for EnchantmentEffectComponentType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_effect_component_type");
}
impl StaticRegistry for EnchantmentEffectComponentType {
    const NAMES: &'static [&'static str] = crate::enchantment_effect_component_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentEntityEffectType {}
impl RegistryKey for EnchantmentEntityEffectType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_entity_effect_type");
}
impl StaticRegistry for EnchantmentEntityEffectType {
    const NAMES: &'static [&'static str] = crate::enchantment_entity_effect_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentLevelBasedValueType {}
impl RegistryKey for EnchantmentLevelBasedValueType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_level_based_value_type");
}
impl StaticRegistry for EnchantmentLevelBasedValueType {
    const NAMES: &'static [&'static str] = crate::enchantment_level_based_value_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentLocationBasedEffectType {}
impl RegistryKey for EnchantmentLocationBasedEffectType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_location_based_effect_type");
}
impl StaticRegistry for EnchantmentLocationBasedEffectType {
    const NAMES: &'static [&'static str] = crate::enchantment_location_based_effect_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentProvider {}
impl RegistryKey for EnchantmentProvider {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_provider");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentProviderType {}
impl RegistryKey for EnchantmentProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_provider_type");
}
impl StaticRegistry for EnchantmentProviderType {
    const NAMES: &'static [&'static str] = crate::enchantment_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentValueEffectType {}
impl RegistryKey for EnchantmentValueEffectType {
    const KEY: StaticResourceLocation = rl!("minecraft:enchantment_value_effect_type");
}
impl StaticRegistry for EnchantmentValueEffectType {
    const NAMES: &'static [&'static str] = crate::enchantment_value_effect_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntitySubPredicateType {}
impl RegistryKey for EntitySubPredicateType {
    const KEY: StaticResourceLocation = rl!("minecraft:entity_sub_predicate_type");
}
impl StaticRegistry for EntitySubPredicateType {
    const NAMES: &'static [&'static str] = crate::entity_sub_predicate_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {}
impl RegistryKey for EntityType {
    const KEY: StaticResourceLocation = rl!("minecraft:entity_type");
}
impl StaticRegistry for EntityType {
    const NAMES: &'static [&'static str] = crate::entity_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvironmentAttribute {}
impl RegistryKey for EnvironmentAttribute {
    const KEY: StaticResourceLocation = rl!("minecraft:environment_attribute");
}
impl StaticRegistry for EnvironmentAttribute {
    const NAMES: &'static [&'static str] = crate::environment_attribute::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatProviderType {}
impl RegistryKey for FloatProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:float_provider_type");
}
impl StaticRegistry for FloatProviderType {
    const NAMES: &'static [&'static str] = crate::float_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fluid {}
impl RegistryKey for Fluid {
    const KEY: StaticResourceLocation = rl!("minecraft:fluid");
}
impl StaticRegistry for Fluid {
    const NAMES: &'static [&'static str] = crate::fluid::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrogVariant {}
impl RegistryKey for FrogVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:frog_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEvent {}
impl RegistryKey for GameEvent {
    const KEY: StaticResourceLocation = rl!("minecraft:game_event");
}
impl StaticRegistry for GameEvent {
    const NAMES: &'static [&'static str] = crate::game_event::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameRule {}
impl RegistryKey for GameRule {
    const KEY: StaticResourceLocation = rl!("minecraft:game_rule");
}
impl StaticRegistry for GameRule {
    const NAMES: &'static [&'static str] = crate::game_rule::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightProviderType {}
impl RegistryKey for HeightProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:height_provider_type");
}
impl StaticRegistry for HeightProviderType {
    const NAMES: &'static [&'static str] = crate::height_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomingRpcMethods {}
impl RegistryKey for IncomingRpcMethods {
    const KEY: StaticResourceLocation = rl!("minecraft:incoming_rpc_methods");
}
impl StaticRegistry for IncomingRpcMethods {
    const NAMES: &'static [&'static str] = crate::incoming_rpc_methods::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputControlType {}
impl RegistryKey for InputControlType {
    const KEY: StaticResourceLocation = rl!("minecraft:input_control_type");
}
impl StaticRegistry for InputControlType {
    const NAMES: &'static [&'static str] = crate::input_control_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instrument {}
impl RegistryKey for Instrument {
    const KEY: StaticResourceLocation = rl!("minecraft:instrument");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntProviderType {}
impl RegistryKey for IntProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:int_provider_type");
}
impl StaticRegistry for IntProviderType {
    const NAMES: &'static [&'static str] = crate::int_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {}
impl RegistryKey for Item {
    const KEY: StaticResourceLocation = rl!("minecraft:item");
}
impl StaticRegistry for Item {
    const NAMES: &'static [&'static str] = crate::item::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemModifier {}
impl RegistryKey for ItemModifier {
    const KEY: StaticResourceLocation = rl!("minecraft:item_modifier");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JukeboxSong {}
impl RegistryKey for JukeboxSong {
    const KEY: StaticResourceLocation = rl!("minecraft:jukebox_song");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootConditionType {}
impl RegistryKey for LootConditionType {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_condition_type");
}
impl StaticRegistry for LootConditionType {
    const NAMES: &'static [&'static str] = crate::loot_condition_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootFunctionType {}
impl RegistryKey for LootFunctionType {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_function_type");
}
impl StaticRegistry for LootFunctionType {
    const NAMES: &'static [&'static str] = crate::loot_function_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootNbtProviderType {}
impl RegistryKey for LootNbtProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_nbt_provider_type");
}
impl StaticRegistry for LootNbtProviderType {
    const NAMES: &'static [&'static str] = crate::loot_nbt_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootPoolEntryType {}
impl RegistryKey for LootPoolEntryType {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_pool_entry_type");
}
impl StaticRegistry for LootPoolEntryType {
    const NAMES: &'static [&'static str] = crate::loot_pool_entry_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootScoreProviderType {}
impl RegistryKey for LootScoreProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_score_provider_type");
}
impl StaticRegistry for LootScoreProviderType {
    const NAMES: &'static [&'static str] = crate::loot_score_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootTable {}
impl RegistryKey for LootTable {
    const KEY: StaticResourceLocation = rl!("minecraft:loot_table");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapDecorationType {}
impl RegistryKey for MapDecorationType {
    const KEY: StaticResourceLocation = rl!("minecraft:map_decoration_type");
}
impl StaticRegistry for MapDecorationType {
    const NAMES: &'static [&'static str] = crate::map_decoration_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryModuleType {}
impl RegistryKey for MemoryModuleType {
    const KEY: StaticResourceLocation = rl!("minecraft:memory_module_type");
}
impl StaticRegistry for MemoryModuleType {
    const NAMES: &'static [&'static str] = crate::memory_module_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Menu {}
impl RegistryKey for Menu {
    const KEY: StaticResourceLocation = rl!("minecraft:menu");
}
impl StaticRegistry for Menu {
    const NAMES: &'static [&'static str] = crate::menu::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobEffect {}
impl RegistryKey for MobEffect {
    const KEY: StaticResourceLocation = rl!("minecraft:mob_effect");
}
impl StaticRegistry for MobEffect {
    const NAMES: &'static [&'static str] = crate::mob_effect::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberFormatType {}
impl RegistryKey for NumberFormatType {
    const KEY: StaticResourceLocation = rl!("minecraft:number_format_type");
}
impl StaticRegistry for NumberFormatType {
    const NAMES: &'static [&'static str] = crate::number_format_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutgoingRpcMethods {}
impl RegistryKey for OutgoingRpcMethods {
    const KEY: StaticResourceLocation = rl!("minecraft:outgoing_rpc_methods");
}
impl StaticRegistry for OutgoingRpcMethods {
    const NAMES: &'static [&'static str] = crate::outgoing_rpc_methods::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaintingVariant {}
impl RegistryKey for PaintingVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:painting_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticleType {}
impl RegistryKey for ParticleType {
    const KEY: StaticResourceLocation = rl!("minecraft:particle_type");
}
impl StaticRegistry for ParticleType {
    const NAMES: &'static [&'static str] = crate::particle_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionCheckType {}
impl RegistryKey for PermissionCheckType {
    const KEY: StaticResourceLocation = rl!("minecraft:permission_check_type");
}
impl StaticRegistry for PermissionCheckType {
    const NAMES: &'static [&'static str] = crate::permission_check_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {}
impl RegistryKey for PermissionType {
    const KEY: StaticResourceLocation = rl!("minecraft:permission_type");
}
impl StaticRegistry for PermissionType {
    const NAMES: &'static [&'static str] = crate::permission_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PigSoundVariant {}
impl RegistryKey for PigSoundVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:pig_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PigVariant {}
impl RegistryKey for PigVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:pig_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointOfInterestType {}
impl RegistryKey for PointOfInterestType {
    const KEY: StaticResourceLocation = rl!("minecraft:point_of_interest_type");
}
impl StaticRegistry for PointOfInterestType {
    const NAMES: &'static [&'static str] = crate::point_of_interest_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PosRuleTest {}
impl RegistryKey for PosRuleTest {
    const KEY: StaticResourceLocation = rl!("minecraft:pos_rule_test");
}
impl StaticRegistry for PosRuleTest {
    const NAMES: &'static [&'static str] = crate::pos_rule_test::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionSourceType {}
impl RegistryKey for PositionSourceType {
    const KEY: StaticResourceLocation = rl!("minecraft:position_source_type");
}
impl StaticRegistry for PositionSourceType {
    const NAMES: &'static [&'static str] = crate::position_source_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Potion {}
impl RegistryKey for Potion {
    const KEY: StaticResourceLocation = rl!("minecraft:potion");
}
impl StaticRegistry for Potion {
    const NAMES: &'static [&'static str] = crate::potion::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Predicate {}
impl RegistryKey for Predicate {
    const KEY: StaticResourceLocation = rl!("minecraft:predicate");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recipe {}
impl RegistryKey for Recipe {
    const KEY: StaticResourceLocation = rl!("minecraft:recipe");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeBookCategory {}
impl RegistryKey for RecipeBookCategory {
    const KEY: StaticResourceLocation = rl!("minecraft:recipe_book_category");
}
impl StaticRegistry for RecipeBookCategory {
    const NAMES: &'static [&'static str] = crate::recipe_book_category::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeDisplay {}
impl RegistryKey for RecipeDisplay {
    const KEY: StaticResourceLocation = rl!("minecraft:recipe_display");
}
impl StaticRegistry for RecipeDisplay {
    const NAMES: &'static [&'static str] = crate::recipe_display::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeSerializer {}
impl RegistryKey for RecipeSerializer {
    const KEY: StaticResourceLocation = rl!("minecraft:recipe_serializer");
}
impl StaticRegistry for RecipeSerializer {
    const NAMES: &'static [&'static str] = crate::recipe_serializer::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeType {}
impl RegistryKey for RecipeType {
    const KEY: StaticResourceLocation = rl!("minecraft:recipe_type");
}
impl StaticRegistry for RecipeType {
    const NAMES: &'static [&'static str] = crate::recipe_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleBlockEntityModifier {}
impl RegistryKey for RuleBlockEntityModifier {
    const KEY: StaticResourceLocation = rl!("minecraft:rule_block_entity_modifier");
}
impl StaticRegistry for RuleBlockEntityModifier {
    const NAMES: &'static [&'static str] = crate::rule_block_entity_modifier::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleTestType {}
impl RegistryKey for RuleTestType {
    const KEY: StaticResourceLocation = rl!("minecraft:rule_test_type");
}
impl StaticRegistry for RuleTestType {
    const NAMES: &'static [&'static str] = crate::rule_test_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorType {}
impl RegistryKey for SensorType {
    const KEY: StaticResourceLocation = rl!("minecraft:sensor_type");
}
impl StaticRegistry for SensorType {
    const NAMES: &'static [&'static str] = crate::sensor_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotDisplay {}
impl RegistryKey for SlotDisplay {
    const KEY: StaticResourceLocation = rl!("minecraft:slot_display");
}
impl StaticRegistry for SlotDisplay {
    const NAMES: &'static [&'static str] = crate::slot_display::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotSource {}
impl RegistryKey for SlotSource {
    const KEY: StaticResourceLocation = rl!("minecraft:slot_source");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotSourceType {}
impl RegistryKey for SlotSourceType {
    const KEY: StaticResourceLocation = rl!("minecraft:slot_source_type");
}
impl StaticRegistry for SlotSourceType {
    const NAMES: &'static [&'static str] = crate::slot_source_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEvent {}
impl RegistryKey for SoundEvent {
    const KEY: StaticResourceLocation = rl!("minecraft:sound_event");
}
impl StaticRegistry for SoundEvent {
    const NAMES: &'static [&'static str] = crate::sound_event::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnConditionType {}
impl RegistryKey for SpawnConditionType {
    const KEY: StaticResourceLocation = rl!("minecraft:spawn_condition_type");
}
impl StaticRegistry for SpawnConditionType {
    const NAMES: &'static [&'static str] = crate::spawn_condition_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatType {}
impl RegistryKey for StatType {
    const KEY: StaticResourceLocation = rl!("minecraft:stat_type");
}
impl StaticRegistry for StatType {
    const NAMES: &'static [&'static str] = crate::stat_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SulfurCubeArchetype {}
impl RegistryKey for SulfurCubeArchetype {
    const KEY: StaticResourceLocation = rl!("minecraft:sulfur_cube_archetype");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestEnvironment {}
impl RegistryKey for TestEnvironment {
    const KEY: StaticResourceLocation = rl!("minecraft:test_environment");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestEnvironmentDefinitionType {}
impl RegistryKey for TestEnvironmentDefinitionType {
    const KEY: StaticResourceLocation = rl!("minecraft:test_environment_definition_type");
}
impl StaticRegistry for TestEnvironmentDefinitionType {
    const NAMES: &'static [&'static str] = crate::test_environment_definition_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestFunction {}
impl RegistryKey for TestFunction {
    const KEY: StaticResourceLocation = rl!("minecraft:test_function");
}
impl StaticRegistry for TestFunction {
    const NAMES: &'static [&'static str] = crate::test_function::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestInstance {}
impl RegistryKey for TestInstance {
    const KEY: StaticResourceLocation = rl!("minecraft:test_instance");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestInstanceType {}
impl RegistryKey for TestInstanceType {
    const KEY: StaticResourceLocation = rl!("minecraft:test_instance_type");
}
impl StaticRegistry for TestInstanceType {
    const NAMES: &'static [&'static str] = crate::test_instance_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TicketType {}
impl RegistryKey for TicketType {
    const KEY: StaticResourceLocation = rl!("minecraft:ticket_type");
}
impl StaticRegistry for TicketType {
    const NAMES: &'static [&'static str] = crate::ticket_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeline {}
impl RegistryKey for Timeline {
    const KEY: StaticResourceLocation = rl!("minecraft:timeline");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeSet {}
impl RegistryKey for TradeSet {
    const KEY: StaticResourceLocation = rl!("minecraft:trade_set");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrialSpawner {}
impl RegistryKey for TrialSpawner {
    const KEY: StaticResourceLocation = rl!("minecraft:trial_spawner");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {}
impl RegistryKey for TriggerType {
    const KEY: StaticResourceLocation = rl!("minecraft:trigger_type");
}
impl StaticRegistry for TriggerType {
    const NAMES: &'static [&'static str] = crate::trigger_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrimMaterial {}
impl RegistryKey for TrimMaterial {
    const KEY: StaticResourceLocation = rl!("minecraft:trim_material");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrimPattern {}
impl RegistryKey for TrimPattern {
    const KEY: StaticResourceLocation = rl!("minecraft:trim_pattern");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerProfession {}
impl RegistryKey for VillagerProfession {
    const KEY: StaticResourceLocation = rl!("minecraft:villager_profession");
}
impl StaticRegistry for VillagerProfession {
    const NAMES: &'static [&'static str] = crate::villager_profession::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerTrade {}
impl RegistryKey for VillagerTrade {
    const KEY: StaticResourceLocation = rl!("minecraft:villager_trade");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerType {}
impl RegistryKey for VillagerType {
    const KEY: StaticResourceLocation = rl!("minecraft:villager_type");
}
impl StaticRegistry for VillagerType {
    const NAMES: &'static [&'static str] = crate::villager_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfSoundVariant {}
impl RegistryKey for WolfSoundVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:wolf_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfVariant {}
impl RegistryKey for WolfVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:wolf_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldClock {}
impl RegistryKey for WorldClock {
    const KEY: StaticResourceLocation = rl!("minecraft:world_clock");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {}
impl RegistryKey for Biome {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/biome");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeSource {}
impl RegistryKey for BiomeSource {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/biome_source");
}
impl StaticRegistry for BiomeSource {
    const NAMES: &'static [&'static str] = crate::biome_source::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockStateProvider {}
impl RegistryKey for BlockStateProvider {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/block_state_provider");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockStateProviderType {}
impl RegistryKey for BlockStateProviderType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/block_state_provider_type");
}
impl StaticRegistry for BlockStateProviderType {
    const NAMES: &'static [&'static str] = crate::block_state_provider_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Carver {}
impl RegistryKey for Carver {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/carver");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarverType {}
impl RegistryKey for CarverType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/carver_type");
}
impl StaticRegistry for CarverType {
    const NAMES: &'static [&'static str] = crate::carver_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkGenerator {}
impl RegistryKey for ChunkGenerator {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/chunk_generator");
}
impl StaticRegistry for ChunkGenerator {
    const NAMES: &'static [&'static str] = crate::chunk_generator::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DensityFunction {}
impl RegistryKey for DensityFunction {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/density_function");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DensityFunctionType {}
impl RegistryKey for DensityFunctionType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/density_function_type");
}
impl StaticRegistry for DensityFunctionType {
    const NAMES: &'static [&'static str] = crate::density_function_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {}
impl RegistryKey for Feature {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/feature");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureSizeType {}
impl RegistryKey for FeatureSizeType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/feature_size_type");
}
impl StaticRegistry for FeatureSizeType {
    const NAMES: &'static [&'static str] = crate::feature_size_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureType {}
impl RegistryKey for FeatureType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/feature_type");
}
impl StaticRegistry for FeatureType {
    const NAMES: &'static [&'static str] = crate::feature_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlatLevelGeneratorPreset {}
impl RegistryKey for FlatLevelGeneratorPreset {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/flat_level_generator_preset");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoliagePlacerType {}
impl RegistryKey for FoliagePlacerType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/foliage_placer_type");
}
impl StaticRegistry for FoliagePlacerType {
    const NAMES: &'static [&'static str] = crate::foliage_placer_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialCondition {}
impl RegistryKey for MaterialCondition {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/material_condition");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialConditionType {}
impl RegistryKey for MaterialConditionType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/material_condition_type");
}
impl StaticRegistry for MaterialConditionType {
    const NAMES: &'static [&'static str] = crate::material_condition_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialRule {}
impl RegistryKey for MaterialRule {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/material_rule");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialRuleType {}
impl RegistryKey for MaterialRuleType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/material_rule_type");
}
impl StaticRegistry for MaterialRuleType {
    const NAMES: &'static [&'static str] = crate::material_rule_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiNoiseBiomeSourceParameterList {}
impl RegistryKey for MultiNoiseBiomeSourceParameterList {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/multi_noise_biome_source_parameter_list");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Noise {}
impl RegistryKey for Noise {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/noise");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseSettings {}
impl RegistryKey for NoiseSettings {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/noise_settings");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacedFeature {}
impl RegistryKey for PlacedFeature {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/placed_feature");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacementModifierType {}
impl RegistryKey for PlacementModifierType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/placement_modifier_type");
}
impl StaticRegistry for PlacementModifierType {
    const NAMES: &'static [&'static str] = crate::placement_modifier_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolAliasBinding {}
impl RegistryKey for PoolAliasBinding {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/pool_alias_binding");
}
impl StaticRegistry for PoolAliasBinding {
    const NAMES: &'static [&'static str] = crate::pool_alias_binding::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessorList {}
impl RegistryKey for ProcessorList {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/processor_list");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootPlacerType {}
impl RegistryKey for RootPlacerType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/root_placer_type");
}
impl StaticRegistry for RootPlacerType {
    const NAMES: &'static [&'static str] = crate::root_placer_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Structure {}
impl RegistryKey for Structure {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePiece {}
impl RegistryKey for StructurePiece {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_piece");
}
impl StaticRegistry for StructurePiece {
    const NAMES: &'static [&'static str] = crate::structure_piece::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePlacement {}
impl RegistryKey for StructurePlacement {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_placement");
}
impl StaticRegistry for StructurePlacement {
    const NAMES: &'static [&'static str] = crate::structure_placement::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePoolElement {}
impl RegistryKey for StructurePoolElement {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_pool_element");
}
impl StaticRegistry for StructurePoolElement {
    const NAMES: &'static [&'static str] = crate::structure_pool_element::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureProcessor {}
impl RegistryKey for StructureProcessor {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_processor");
}
impl StaticRegistry for StructureProcessor {
    const NAMES: &'static [&'static str] = crate::structure_processor::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureSet {}
impl RegistryKey for StructureSet {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_set");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureType {}
impl RegistryKey for StructureType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/structure_type");
}
impl StaticRegistry for StructureType {
    const NAMES: &'static [&'static str] = crate::structure_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplatePool {}
impl RegistryKey for TemplatePool {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/template_pool");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeDecoratorType {}
impl RegistryKey for TreeDecoratorType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/tree_decorator_type");
}
impl StaticRegistry for TreeDecoratorType {
    const NAMES: &'static [&'static str] = crate::tree_decorator_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrunkPlacerType {}
impl RegistryKey for TrunkPlacerType {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/trunk_placer_type");
}
impl StaticRegistry for TrunkPlacerType {
    const NAMES: &'static [&'static str] = crate::trunk_placer_type::NAMES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldPreset {}
impl RegistryKey for WorldPreset {
    const KEY: StaticResourceLocation = rl!("minecraft:worldgen/world_preset");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZombieNautilusVariant {}
impl RegistryKey for ZombieNautilusVariant {
    const KEY: StaticResourceLocation = rl!("minecraft:zombie_nautilus_variant");
}
