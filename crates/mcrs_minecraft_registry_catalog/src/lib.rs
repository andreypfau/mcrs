// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, TypeBinding};

#[rustfmt::skip]
pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[
    (mcrs_minecraft_environment::keys::ACTIVITY.location(), mcrs_minecraft_environment::keys::Activity::ENTRIES),
    (mcrs_minecraft_entity::keys::ATTRIBUTE.location(), mcrs_minecraft_entity::keys::Attribute::ENTRIES),
    (mcrs_minecraft_keys::ATTRIBUTE_TYPE.location(), mcrs_minecraft_keys::attribute_type::ENTRIES),
    (mcrs_minecraft_block::keys::BLOCK.location(), mcrs_minecraft_block::keys::Block::ENTRIES),
    (mcrs_minecraft_block::keys::BLOCK_ENTITY_TYPE.location(), mcrs_minecraft_block::keys::BlockEntityType::ENTRIES),
    (mcrs_minecraft_block_predicate::keys::BLOCK_PREDICATE_TYPE.location(), mcrs_minecraft_block_predicate::keys::BlockPredicateType::ENTRIES),
    (mcrs_minecraft_anvil::keys::CHUNK_STATUS.location(), mcrs_minecraft_anvil::keys::ChunkStatus::ENTRIES),
    (mcrs_minecraft_keys::COMMAND_ARGUMENT_TYPE.location(), mcrs_minecraft_keys::command_argument_type::ENTRIES),
    (mcrs_minecraft_item::keys::CONSUME_EFFECT_TYPE.location(), mcrs_minecraft_item::keys::ConsumeEffectType::ENTRIES),
    (mcrs_minecraft_keys::CONTEXT_FLOAT_PROVIDER_TYPE.location(), mcrs_minecraft_keys::context_float_provider_type::ENTRIES),
    (mcrs_minecraft_keys::CONTEXT_INT_PROVIDER_TYPE.location(), mcrs_minecraft_keys::context_int_provider_type::ENTRIES),
    (mcrs_minecraft_keys::CONTEXT_KEY_SET.location(), mcrs_minecraft_keys::context_key_set::ENTRIES),
    (mcrs_minecraft_keys::CREATIVE_MODE_TAB.location(), mcrs_minecraft_keys::creative_mode_tab::ENTRIES),
    (mcrs_minecraft_keys::CUSTOM_STAT.location(), mcrs_minecraft_keys::custom_stat::ENTRIES),
    (mcrs_minecraft_item::keys::DATA_COMPONENT_PREDICATE_TYPE.location(), mcrs_minecraft_item::keys::DataComponentPredicateType::ENTRIES),
    (mcrs_minecraft_item::keys::DATA_COMPONENT_TYPE.location(), mcrs_minecraft_item::keys::DataComponentType::ENTRIES),
    (mcrs_minecraft_keys::DEBUG_SUBSCRIPTION.location(), mcrs_minecraft_keys::debug_subscription::ENTRIES),
    (mcrs_minecraft_keys::DIALOG_ACTION_TYPE.location(), mcrs_minecraft_keys::dialog_action_type::ENTRIES),
    (mcrs_minecraft_item::keys::DIALOG_BODY_TYPE.location(), mcrs_minecraft_item::keys::DialogBodyType::ENTRIES),
    (mcrs_minecraft_item::keys::DIALOG_TYPE.location(), mcrs_minecraft_item::keys::DialogType::ENTRIES),
    (mcrs_minecraft_enchantment::keys::ENCHANTMENT_EFFECT_COMPONENT_TYPE.location(), mcrs_minecraft_enchantment::keys::EnchantmentEffectComponentType::ENTRIES),
    (mcrs_minecraft_enchantment::keys::ENCHANTMENT_ENTITY_EFFECT_TYPE.location(), mcrs_minecraft_enchantment::keys::EnchantmentEntityEffectType::ENTRIES),
    (mcrs_minecraft_enchantment::keys::ENCHANTMENT_LEVEL_BASED_VALUE_TYPE.location(), mcrs_minecraft_enchantment::keys::EnchantmentLevelBasedValueType::ENTRIES),
    (mcrs_minecraft_enchantment::keys::ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE.location(), mcrs_minecraft_enchantment::keys::EnchantmentLocationBasedEffectType::ENTRIES),
    (mcrs_minecraft_keys::ENCHANTMENT_PROVIDER_TYPE.location(), mcrs_minecraft_keys::enchantment_provider_type::ENTRIES),
    (mcrs_minecraft_enchantment::keys::ENCHANTMENT_VALUE_EFFECT_TYPE.location(), mcrs_minecraft_enchantment::keys::EnchantmentValueEffectType::ENTRIES),
    (mcrs_minecraft_keys::ENTITY_SUB_PREDICATE_TYPE.location(), mcrs_minecraft_keys::entity_sub_predicate_type::ENTRIES),
    (mcrs_minecraft_entity::keys::ENTITY_TYPE.location(), mcrs_minecraft_entity::keys::EntityType::ENTRIES),
    (mcrs_minecraft_environment::keys::ENVIRONMENT_ATTRIBUTE.location(), mcrs_minecraft_environment::keys::EnvironmentAttribute::ENTRIES),
    (mcrs_minecraft_value_provider::keys::FLOAT_PROVIDER_TYPE.location(), mcrs_minecraft_value_provider::keys::FloatProviderType::ENTRIES),
    (mcrs_minecraft_block::keys::FLUID.location(), mcrs_minecraft_block::keys::Fluid::ENTRIES),
    (mcrs_minecraft_keys::GAME_EVENT.location(), mcrs_minecraft_keys::game_event::ENTRIES),
    (mcrs_minecraft_keys::GAME_RULE.location(), mcrs_minecraft_keys::game_rule::ENTRIES),
    (mcrs_minecraft_value_provider::keys::HEIGHT_PROVIDER_TYPE.location(), mcrs_minecraft_value_provider::keys::HeightProviderType::ENTRIES),
    (mcrs_minecraft_keys::INCOMING_RPC_METHODS.location(), mcrs_minecraft_keys::incoming_rpc_methods::ENTRIES),
    (mcrs_minecraft_item::keys::INPUT_CONTROL_TYPE.location(), mcrs_minecraft_item::keys::InputControlType::ENTRIES),
    (mcrs_minecraft_value_provider::keys::INT_PROVIDER_TYPE.location(), mcrs_minecraft_value_provider::keys::IntProviderType::ENTRIES),
    (mcrs_minecraft_item::keys::ITEM.location(), mcrs_minecraft_item::keys::Item::ENTRIES),
    (mcrs_minecraft_enchantment::keys::LOOT_CONDITION_TYPE.location(), mcrs_minecraft_enchantment::keys::LootConditionType::ENTRIES),
    (mcrs_minecraft_keys::LOOT_FUNCTION_TYPE.location(), mcrs_minecraft_keys::loot_function_type::ENTRIES),
    (mcrs_minecraft_keys::LOOT_NBT_PROVIDER_TYPE.location(), mcrs_minecraft_keys::loot_nbt_provider_type::ENTRIES),
    (mcrs_minecraft_keys::LOOT_POOL_ENTRY_TYPE.location(), mcrs_minecraft_keys::loot_pool_entry_type::ENTRIES),
    (mcrs_minecraft_keys::LOOT_SCORE_PROVIDER_TYPE.location(), mcrs_minecraft_keys::loot_score_provider_type::ENTRIES),
    (mcrs_minecraft_item::keys::MAP_DECORATION_TYPE.location(), mcrs_minecraft_item::keys::MapDecorationType::ENTRIES),
    (mcrs_minecraft_keys::MEMORY_MODULE_TYPE.location(), mcrs_minecraft_keys::memory_module_type::ENTRIES),
    (mcrs_minecraft_item::keys::MENU.location(), mcrs_minecraft_item::keys::MenuType::ENTRIES),
    (mcrs_minecraft_item::keys::MOB_EFFECT.location(), mcrs_minecraft_item::keys::MobEffect::ENTRIES),
    (mcrs_minecraft_keys::NUMBER_FORMAT_TYPE.location(), mcrs_minecraft_keys::number_format_type::ENTRIES),
    (mcrs_minecraft_keys::OUTGOING_RPC_METHODS.location(), mcrs_minecraft_keys::outgoing_rpc_methods::ENTRIES),
    (mcrs_minecraft_particle::keys::PARTICLE_TYPE.location(), mcrs_minecraft_particle::keys::ParticleType::ENTRIES),
    (mcrs_minecraft_keys::PERMISSION_CHECK_TYPE.location(), mcrs_minecraft_keys::permission_check_type::ENTRIES),
    (mcrs_minecraft_keys::PERMISSION_TYPE.location(), mcrs_minecraft_keys::permission_type::ENTRIES),
    (mcrs_minecraft_keys::POINT_OF_INTEREST_TYPE.location(), mcrs_minecraft_keys::point_of_interest_type::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::POS_RULE_TEST.location(), mcrs_minecraft_worldgen_feature::keys::PosRuleTestType::ENTRIES),
    (mcrs_minecraft_particle::keys::POSITION_SOURCE_TYPE.location(), mcrs_minecraft_particle::keys::PositionSourceType::ENTRIES),
    (mcrs_minecraft_item::keys::POTION.location(), mcrs_minecraft_item::keys::Potion::ENTRIES),
    (mcrs_minecraft_keys::RECIPE_BOOK_CATEGORY.location(), mcrs_minecraft_keys::recipe_book_category::ENTRIES),
    (mcrs_minecraft_protocol::keys::RECIPE_DISPLAY.location(), mcrs_minecraft_protocol::keys::RecipeDisplayType::ENTRIES),
    (mcrs_minecraft_keys::RECIPE_SERIALIZER.location(), mcrs_minecraft_keys::recipe_serializer::ENTRIES),
    (mcrs_minecraft_keys::RECIPE_TYPE.location(), mcrs_minecraft_keys::recipe_type::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::RULE_BLOCK_ENTITY_MODIFIER.location(), mcrs_minecraft_worldgen_feature::keys::RuleBlockEntityModifierType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::RULE_TEST_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::RuleTestType::ENTRIES),
    (mcrs_minecraft_keys::SENSOR_TYPE.location(), mcrs_minecraft_keys::sensor_type::ENTRIES),
    (mcrs_minecraft_protocol::keys::SLOT_DISPLAY.location(), mcrs_minecraft_protocol::keys::SlotDisplayType::ENTRIES),
    (mcrs_minecraft_keys::SLOT_SOURCE_TYPE.location(), mcrs_minecraft_keys::slot_source_type::ENTRIES),
    (mcrs_minecraft_sound::keys::SOUND_EVENT.location(), mcrs_minecraft_sound::keys::sound_event::ENTRIES),
    (mcrs_minecraft_worldgen_structure::keys::SPAWN_CONDITION_TYPE.location(), mcrs_minecraft_worldgen_structure::keys::SpawnConditionType::ENTRIES),
    (mcrs_minecraft_keys::STAT_TYPE.location(), mcrs_minecraft_keys::stat_type::ENTRIES),
    (mcrs_minecraft_keys::TEST_ENVIRONMENT_DEFINITION_TYPE.location(), mcrs_minecraft_keys::test_environment_definition_type::ENTRIES),
    (mcrs_minecraft_keys::TEST_FUNCTION.location(), mcrs_minecraft_keys::test_function::ENTRIES),
    (mcrs_minecraft_keys::TEST_INSTANCE_TYPE.location(), mcrs_minecraft_keys::test_instance_type::ENTRIES),
    (mcrs_minecraft_keys::TICKET_TYPE.location(), mcrs_minecraft_keys::ticket_type::ENTRIES),
    (mcrs_minecraft_keys::TRIGGER_TYPE.location(), mcrs_minecraft_keys::trigger_type::ENTRIES),
    (mcrs_minecraft_entity::keys::VILLAGER_PROFESSION.location(), mcrs_minecraft_entity::keys::VillagerProfession::ENTRIES),
    (mcrs_minecraft_entity::keys::VILLAGER_TYPE.location(), mcrs_minecraft_entity::keys::VillagerType::ENTRIES),
    (mcrs_minecraft_biome::keys::BIOME_SOURCE.location(), mcrs_minecraft_biome::keys::BiomeSourceType::ENTRIES),
    (mcrs_minecraft_block_predicate::keys::BLOCK_STATE_PROVIDER_TYPE.location(), mcrs_minecraft_block_predicate::keys::BlockStateProviderType::ENTRIES),
    (mcrs_minecraft_worldgen_carver::keys::CARVER_TYPE.location(), mcrs_minecraft_worldgen_carver::keys::CarverType::ENTRIES),
    (mcrs_minecraft_keys::CHUNK_GENERATOR.location(), mcrs_minecraft_keys::chunk_generator::ENTRIES),
    (mcrs_minecraft_worldgen_density::keys::DENSITY_FUNCTION_TYPE.location(), mcrs_minecraft_worldgen_density::keys::DensityFunctionType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::FEATURE_SIZE_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::FeatureSizeType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::FEATURE_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::FeatureType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::FOLIAGE_PLACER_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::FoliagePlacerType::ENTRIES),
    (mcrs_minecraft_worldgen_surface::keys::MATERIAL_CONDITION_TYPE.location(), mcrs_minecraft_worldgen_surface::keys::MaterialConditionType::ENTRIES),
    (mcrs_minecraft_worldgen_surface::keys::MATERIAL_RULE_TYPE.location(), mcrs_minecraft_worldgen_surface::keys::MaterialRuleType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::PLACEMENT_MODIFIER_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::PlacementModifierType::ENTRIES),
    (mcrs_minecraft_worldgen_structure::keys::POOL_ALIAS_BINDING.location(), mcrs_minecraft_worldgen_structure::keys::PoolAliasBindingType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::ROOT_PLACER_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::RootPlacerType::ENTRIES),
    (mcrs_minecraft_worldgen_structure::keys::STRUCTURE_PIECE.location(), mcrs_minecraft_worldgen_structure::keys::StructurePieceType::ENTRIES),
    (mcrs_minecraft_worldgen_structure::keys::STRUCTURE_PLACEMENT.location(), mcrs_minecraft_worldgen_structure::keys::StructurePlacementType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::STRUCTURE_POOL_ELEMENT.location(), mcrs_minecraft_worldgen_feature::keys::StructurePoolElementType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::STRUCTURE_PROCESSOR.location(), mcrs_minecraft_worldgen_feature::keys::StructureProcessorType::ENTRIES),
    (mcrs_minecraft_worldgen_structure::keys::STRUCTURE_TYPE.location(), mcrs_minecraft_worldgen_structure::keys::StructureType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::TREE_DECORATOR_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::TreeDecoratorType::ENTRIES),
    (mcrs_minecraft_worldgen_feature::keys::TRUNK_PLACER_TYPE.location(), mcrs_minecraft_worldgen_feature::keys::TrunkPlacerType::ENTRIES),
];

#[rustfmt::skip]
pub fn bindings() -> impl Iterator<Item = TypeBinding> {
    std::iter::empty()
        .chain(mcrs_minecraft_anvil::keys::bindings())
        .chain(mcrs_minecraft_biome::keys::bindings())
        .chain(mcrs_minecraft_block::keys::bindings())
        .chain(mcrs_minecraft_block_predicate::keys::bindings())
        .chain(mcrs_minecraft_dimension::keys::bindings())
        .chain(mcrs_minecraft_enchantment::keys::bindings())
        .chain(mcrs_minecraft_entity::keys::bindings())
        .chain(mcrs_minecraft_environment::keys::bindings())
        .chain(mcrs_minecraft_item::keys::bindings())
        .chain(mcrs_minecraft_keys::bindings())
        .chain(mcrs_minecraft_particle::keys::bindings())
        .chain(mcrs_minecraft_protocol::keys::bindings())
        .chain(mcrs_minecraft_sound::keys::bindings())
        .chain(mcrs_minecraft_value_provider::keys::bindings())
        .chain(mcrs_minecraft_worldgen_carver::keys::bindings())
        .chain(mcrs_minecraft_worldgen_density::keys::bindings())
        .chain(mcrs_minecraft_worldgen_feature::keys::bindings())
        .chain(mcrs_minecraft_worldgen_noise::keys::bindings())
        .chain(mcrs_minecraft_worldgen_structure::keys::bindings())
        .chain(mcrs_minecraft_worldgen_surface::keys::bindings())
}
