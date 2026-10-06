// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

#[rustfmt::skip]
pub mod activity;
#[rustfmt::skip]
pub mod attribute;
#[rustfmt::skip]
pub mod attribute_type;
#[rustfmt::skip]
pub mod banner_pattern;
#[rustfmt::skip]
pub mod banner_pattern_tags;
#[rustfmt::skip]
pub mod biome;
#[rustfmt::skip]
pub mod biome_source;
#[rustfmt::skip]
pub mod biome_tags;
#[rustfmt::skip]
pub mod block;
#[rustfmt::skip]
pub mod block_entity_type;
#[rustfmt::skip]
pub mod block_predicate_type;
#[rustfmt::skip]
pub mod block_state_provider;
#[rustfmt::skip]
pub mod block_state_provider_type;
#[rustfmt::skip]
pub mod block_tags;
#[rustfmt::skip]
pub mod block_transformer;
#[rustfmt::skip]
pub mod carver;
#[rustfmt::skip]
pub mod carver_type;
#[rustfmt::skip]
pub mod cat_sound_variant;
#[rustfmt::skip]
pub mod cat_variant;
#[rustfmt::skip]
pub mod chat_type;
#[rustfmt::skip]
pub mod chicken_sound_variant;
#[rustfmt::skip]
pub mod chicken_variant;
#[rustfmt::skip]
pub mod chunk_generator;
#[rustfmt::skip]
pub mod chunk_status;
#[rustfmt::skip]
pub mod command_argument_type;
#[rustfmt::skip]
pub mod consume_effect_type;
#[rustfmt::skip]
pub mod context_float_provider;
#[rustfmt::skip]
pub mod context_float_provider_type;
#[rustfmt::skip]
pub mod context_int_provider;
#[rustfmt::skip]
pub mod context_int_provider_type;
#[rustfmt::skip]
pub mod context_key_set;
#[rustfmt::skip]
pub mod cow_sound_variant;
#[rustfmt::skip]
pub mod cow_variant;
#[rustfmt::skip]
pub mod creative_mode_tab;
#[rustfmt::skip]
pub mod custom_stat;
#[rustfmt::skip]
pub mod damage_type;
#[rustfmt::skip]
pub mod damage_type_tags;
#[rustfmt::skip]
pub mod data_component_predicate_type;
#[rustfmt::skip]
pub mod data_component_type;
#[rustfmt::skip]
pub mod debug_subscription;
#[rustfmt::skip]
pub mod decorated_pot_pattern;
#[rustfmt::skip]
pub mod density_function;
#[rustfmt::skip]
pub mod density_function_type;
#[rustfmt::skip]
pub mod dialog;
#[rustfmt::skip]
pub mod dialog_action_type;
#[rustfmt::skip]
pub mod dialog_body_type;
#[rustfmt::skip]
pub mod dialog_tags;
#[rustfmt::skip]
pub mod dialog_type;
#[rustfmt::skip]
pub mod dimension;
#[rustfmt::skip]
pub mod dimension_type;
#[rustfmt::skip]
pub mod enchantment;
#[rustfmt::skip]
pub mod enchantment_effect_component_type;
#[rustfmt::skip]
pub mod enchantment_entity_effect_type;
#[rustfmt::skip]
pub mod enchantment_level_based_value_type;
#[rustfmt::skip]
pub mod enchantment_location_based_effect_type;
#[rustfmt::skip]
pub mod enchantment_provider;
#[rustfmt::skip]
pub mod enchantment_provider_type;
#[rustfmt::skip]
pub mod enchantment_tags;
#[rustfmt::skip]
pub mod enchantment_value_effect_type;
#[rustfmt::skip]
pub mod entity_sub_predicate_type;
#[rustfmt::skip]
pub mod entity_type;
#[rustfmt::skip]
pub mod entity_type_tags;
#[rustfmt::skip]
pub mod environment_attribute;
#[rustfmt::skip]
pub mod feature;
#[rustfmt::skip]
pub mod feature_size_type;
#[rustfmt::skip]
pub mod feature_tags;
#[rustfmt::skip]
pub mod feature_type;
#[rustfmt::skip]
pub mod flat_level_generator_preset;
#[rustfmt::skip]
pub mod flat_level_generator_preset_tags;
#[rustfmt::skip]
pub mod float_provider_type;
#[rustfmt::skip]
pub mod fluid;
#[rustfmt::skip]
pub mod fluid_tags;
#[rustfmt::skip]
pub mod foliage_placer_type;
#[rustfmt::skip]
pub mod frog_variant;
#[rustfmt::skip]
pub mod game_event;
#[rustfmt::skip]
pub mod game_event_tags;
#[rustfmt::skip]
pub mod game_rule;
#[rustfmt::skip]
pub mod height_provider_type;
#[rustfmt::skip]
pub mod incoming_rpc_methods;
#[rustfmt::skip]
pub mod input_control_type;
#[rustfmt::skip]
pub mod instrument;
#[rustfmt::skip]
pub mod instrument_tags;
#[rustfmt::skip]
pub mod int_provider_type;
#[rustfmt::skip]
pub mod item;
#[rustfmt::skip]
pub mod item_tags;
#[rustfmt::skip]
pub mod jukebox_song;
#[rustfmt::skip]
pub mod loot_condition_type;
#[rustfmt::skip]
pub mod loot_function_type;
#[rustfmt::skip]
pub mod loot_nbt_provider_type;
#[rustfmt::skip]
pub mod loot_pool_entry_type;
#[rustfmt::skip]
pub mod loot_score_provider_type;
#[rustfmt::skip]
pub mod loot_table;
#[rustfmt::skip]
pub mod map_decoration_type;
#[rustfmt::skip]
pub mod material_condition;
#[rustfmt::skip]
pub mod material_condition_type;
#[rustfmt::skip]
pub mod material_rule;
#[rustfmt::skip]
pub mod material_rule_type;
#[rustfmt::skip]
pub mod memory_module_type;
#[rustfmt::skip]
pub mod menu;
#[rustfmt::skip]
pub mod mob_effect;
#[rustfmt::skip]
pub mod multi_noise_biome_source_parameter_list;
#[rustfmt::skip]
pub mod noise;
#[rustfmt::skip]
pub mod noise_settings;
#[rustfmt::skip]
pub mod number_format_type;
#[rustfmt::skip]
pub mod outgoing_rpc_methods;
#[rustfmt::skip]
pub mod painting_variant;
#[rustfmt::skip]
pub mod painting_variant_tags;
#[rustfmt::skip]
pub mod particle_type;
#[rustfmt::skip]
pub mod permission_check_type;
#[rustfmt::skip]
pub mod permission_type;
#[rustfmt::skip]
pub mod pig_sound_variant;
#[rustfmt::skip]
pub mod pig_variant;
#[rustfmt::skip]
pub mod placed_feature;
#[rustfmt::skip]
pub mod placement_modifier_type;
#[rustfmt::skip]
pub mod point_of_interest_type;
#[rustfmt::skip]
pub mod point_of_interest_type_tags;
#[rustfmt::skip]
pub mod pool_alias_binding;
#[rustfmt::skip]
pub mod pos_rule_test;
#[rustfmt::skip]
pub mod position_source_type;
#[rustfmt::skip]
pub mod potion;
#[rustfmt::skip]
pub mod potion_tags;
#[rustfmt::skip]
pub mod predicate;
#[rustfmt::skip]
pub mod processor_list;
#[rustfmt::skip]
pub mod recipe_book_category;
#[rustfmt::skip]
pub mod recipe_display;
#[rustfmt::skip]
pub mod recipe_serializer;
#[rustfmt::skip]
pub mod recipe_type;
#[rustfmt::skip]
pub mod registry;
#[rustfmt::skip]
pub mod root_placer_type;
#[rustfmt::skip]
pub mod rule_block_entity_modifier;
#[rustfmt::skip]
pub mod rule_test_type;
#[rustfmt::skip]
pub mod sensor_type;
#[rustfmt::skip]
pub mod slot_display;
#[rustfmt::skip]
pub mod slot_source_type;
#[rustfmt::skip]
pub mod sound_event;
#[rustfmt::skip]
pub mod spawn_condition_type;
#[rustfmt::skip]
pub mod stat_type;
#[rustfmt::skip]
pub mod structure;
#[rustfmt::skip]
pub mod structure_piece;
#[rustfmt::skip]
pub mod structure_placement;
#[rustfmt::skip]
pub mod structure_pool_element;
#[rustfmt::skip]
pub mod structure_processor;
#[rustfmt::skip]
pub mod structure_set;
#[rustfmt::skip]
pub mod structure_tags;
#[rustfmt::skip]
pub mod structure_type;
#[rustfmt::skip]
pub mod sulfur_cube_archetype;
#[rustfmt::skip]
pub mod template_pool;
#[rustfmt::skip]
pub mod test_environment;
#[rustfmt::skip]
pub mod test_environment_definition_type;
#[rustfmt::skip]
pub mod test_function;
#[rustfmt::skip]
pub mod test_instance;
#[rustfmt::skip]
pub mod test_instance_type;
#[rustfmt::skip]
pub mod ticket_type;
#[rustfmt::skip]
pub mod timeline;
#[rustfmt::skip]
pub mod timeline_tags;
#[rustfmt::skip]
pub mod trade_set;
#[rustfmt::skip]
pub mod tree_decorator_type;
#[rustfmt::skip]
pub mod trial_spawner;
#[rustfmt::skip]
pub mod trigger_type;
#[rustfmt::skip]
pub mod trim_material;
#[rustfmt::skip]
pub mod trim_pattern;
#[rustfmt::skip]
pub mod trunk_placer_type;
#[rustfmt::skip]
pub mod villager_profession;
#[rustfmt::skip]
pub mod villager_trade;
#[rustfmt::skip]
pub mod villager_trade_tags;
#[rustfmt::skip]
pub mod villager_type;
#[rustfmt::skip]
pub mod wolf_sound_variant;
#[rustfmt::skip]
pub mod wolf_variant;
#[rustfmt::skip]
pub mod world_clock;
#[rustfmt::skip]
pub mod world_preset;
#[rustfmt::skip]
pub mod world_preset_tags;
#[rustfmt::skip]
pub mod zombie_nautilus_variant;

pub use registry::*;

use mcrs_minecraft_core::StaticResourceLocation;

#[rustfmt::skip]
pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[
    (ACTIVITY.location(), activity::ENTRIES),
    (ATTRIBUTE.location(), attribute::ENTRIES),
    (ATTRIBUTE_TYPE.location(), attribute_type::ENTRIES),
    (BLOCK.location(), block::ENTRIES),
    (BLOCK_ENTITY_TYPE.location(), block_entity_type::ENTRIES),
    (BLOCK_PREDICATE_TYPE.location(), block_predicate_type::ENTRIES),
    (CHUNK_STATUS.location(), chunk_status::ENTRIES),
    (COMMAND_ARGUMENT_TYPE.location(), command_argument_type::ENTRIES),
    (CONSUME_EFFECT_TYPE.location(), consume_effect_type::ENTRIES),
    (CONTEXT_FLOAT_PROVIDER_TYPE.location(), context_float_provider_type::ENTRIES),
    (CONTEXT_INT_PROVIDER_TYPE.location(), context_int_provider_type::ENTRIES),
    (CONTEXT_KEY_SET.location(), context_key_set::ENTRIES),
    (CREATIVE_MODE_TAB.location(), creative_mode_tab::ENTRIES),
    (CUSTOM_STAT.location(), custom_stat::ENTRIES),
    (DATA_COMPONENT_PREDICATE_TYPE.location(), data_component_predicate_type::ENTRIES),
    (DATA_COMPONENT_TYPE.location(), data_component_type::ENTRIES),
    (DEBUG_SUBSCRIPTION.location(), debug_subscription::ENTRIES),
    (DIALOG_ACTION_TYPE.location(), dialog_action_type::ENTRIES),
    (DIALOG_BODY_TYPE.location(), dialog_body_type::ENTRIES),
    (DIALOG_TYPE.location(), dialog_type::ENTRIES),
    (ENCHANTMENT_EFFECT_COMPONENT_TYPE.location(), enchantment_effect_component_type::ENTRIES),
    (ENCHANTMENT_ENTITY_EFFECT_TYPE.location(), enchantment_entity_effect_type::ENTRIES),
    (ENCHANTMENT_LEVEL_BASED_VALUE_TYPE.location(), enchantment_level_based_value_type::ENTRIES),
    (ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE.location(), enchantment_location_based_effect_type::ENTRIES),
    (ENCHANTMENT_PROVIDER_TYPE.location(), enchantment_provider_type::ENTRIES),
    (ENCHANTMENT_VALUE_EFFECT_TYPE.location(), enchantment_value_effect_type::ENTRIES),
    (ENTITY_SUB_PREDICATE_TYPE.location(), entity_sub_predicate_type::ENTRIES),
    (ENTITY_TYPE.location(), entity_type::ENTRIES),
    (ENVIRONMENT_ATTRIBUTE.location(), environment_attribute::ENTRIES),
    (FLOAT_PROVIDER_TYPE.location(), float_provider_type::ENTRIES),
    (FLUID.location(), fluid::ENTRIES),
    (GAME_EVENT.location(), game_event::ENTRIES),
    (GAME_RULE.location(), game_rule::ENTRIES),
    (HEIGHT_PROVIDER_TYPE.location(), height_provider_type::ENTRIES),
    (INCOMING_RPC_METHODS.location(), incoming_rpc_methods::ENTRIES),
    (INPUT_CONTROL_TYPE.location(), input_control_type::ENTRIES),
    (INT_PROVIDER_TYPE.location(), int_provider_type::ENTRIES),
    (ITEM.location(), item::ENTRIES),
    (LOOT_CONDITION_TYPE.location(), loot_condition_type::ENTRIES),
    (LOOT_FUNCTION_TYPE.location(), loot_function_type::ENTRIES),
    (LOOT_NBT_PROVIDER_TYPE.location(), loot_nbt_provider_type::ENTRIES),
    (LOOT_POOL_ENTRY_TYPE.location(), loot_pool_entry_type::ENTRIES),
    (LOOT_SCORE_PROVIDER_TYPE.location(), loot_score_provider_type::ENTRIES),
    (MAP_DECORATION_TYPE.location(), map_decoration_type::ENTRIES),
    (MEMORY_MODULE_TYPE.location(), memory_module_type::ENTRIES),
    (MENU.location(), menu::ENTRIES),
    (MOB_EFFECT.location(), mob_effect::ENTRIES),
    (NUMBER_FORMAT_TYPE.location(), number_format_type::ENTRIES),
    (OUTGOING_RPC_METHODS.location(), outgoing_rpc_methods::ENTRIES),
    (PARTICLE_TYPE.location(), particle_type::ENTRIES),
    (PERMISSION_CHECK_TYPE.location(), permission_check_type::ENTRIES),
    (PERMISSION_TYPE.location(), permission_type::ENTRIES),
    (POINT_OF_INTEREST_TYPE.location(), point_of_interest_type::ENTRIES),
    (POS_RULE_TEST.location(), pos_rule_test::ENTRIES),
    (POSITION_SOURCE_TYPE.location(), position_source_type::ENTRIES),
    (POTION.location(), potion::ENTRIES),
    (RECIPE_BOOK_CATEGORY.location(), recipe_book_category::ENTRIES),
    (RECIPE_DISPLAY.location(), recipe_display::ENTRIES),
    (RECIPE_SERIALIZER.location(), recipe_serializer::ENTRIES),
    (RECIPE_TYPE.location(), recipe_type::ENTRIES),
    (RULE_BLOCK_ENTITY_MODIFIER.location(), rule_block_entity_modifier::ENTRIES),
    (RULE_TEST_TYPE.location(), rule_test_type::ENTRIES),
    (SENSOR_TYPE.location(), sensor_type::ENTRIES),
    (SLOT_DISPLAY.location(), slot_display::ENTRIES),
    (SLOT_SOURCE_TYPE.location(), slot_source_type::ENTRIES),
    (SOUND_EVENT.location(), sound_event::ENTRIES),
    (SPAWN_CONDITION_TYPE.location(), spawn_condition_type::ENTRIES),
    (STAT_TYPE.location(), stat_type::ENTRIES),
    (TEST_ENVIRONMENT_DEFINITION_TYPE.location(), test_environment_definition_type::ENTRIES),
    (TEST_FUNCTION.location(), test_function::ENTRIES),
    (TEST_INSTANCE_TYPE.location(), test_instance_type::ENTRIES),
    (TICKET_TYPE.location(), ticket_type::ENTRIES),
    (TRIGGER_TYPE.location(), trigger_type::ENTRIES),
    (VILLAGER_PROFESSION.location(), villager_profession::ENTRIES),
    (VILLAGER_TYPE.location(), villager_type::ENTRIES),
    (BIOME_SOURCE.location(), biome_source::ENTRIES),
    (BLOCK_STATE_PROVIDER_TYPE.location(), block_state_provider_type::ENTRIES),
    (CARVER_TYPE.location(), carver_type::ENTRIES),
    (CHUNK_GENERATOR.location(), chunk_generator::ENTRIES),
    (DENSITY_FUNCTION_TYPE.location(), density_function_type::ENTRIES),
    (FEATURE_SIZE_TYPE.location(), feature_size_type::ENTRIES),
    (FEATURE_TYPE.location(), feature_type::ENTRIES),
    (FOLIAGE_PLACER_TYPE.location(), foliage_placer_type::ENTRIES),
    (MATERIAL_CONDITION_TYPE.location(), material_condition_type::ENTRIES),
    (MATERIAL_RULE_TYPE.location(), material_rule_type::ENTRIES),
    (PLACEMENT_MODIFIER_TYPE.location(), placement_modifier_type::ENTRIES),
    (POOL_ALIAS_BINDING.location(), pool_alias_binding::ENTRIES),
    (ROOT_PLACER_TYPE.location(), root_placer_type::ENTRIES),
    (STRUCTURE_PIECE.location(), structure_piece::ENTRIES),
    (STRUCTURE_PLACEMENT.location(), structure_placement::ENTRIES),
    (STRUCTURE_POOL_ELEMENT.location(), structure_pool_element::ENTRIES),
    (STRUCTURE_PROCESSOR.location(), structure_processor::ENTRIES),
    (STRUCTURE_TYPE.location(), structure_type::ENTRIES),
    (TREE_DECORATOR_TYPE.location(), tree_decorator_type::ENTRIES),
    (TRUNK_PLACER_TYPE.location(), trunk_placer_type::ENTRIES),
];
