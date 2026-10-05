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

#[rustfmt::skip]
pub const STATIC_REGISTRIES: &[(&str, &[&str])] = &[
    ("minecraft:activity", activity::NAMES),
    ("minecraft:attribute", attribute::NAMES),
    ("minecraft:attribute_type", attribute_type::NAMES),
    ("minecraft:block", block::NAMES),
    ("minecraft:block_entity_type", block_entity_type::NAMES),
    ("minecraft:block_predicate_type", block_predicate_type::NAMES),
    ("minecraft:chunk_status", chunk_status::NAMES),
    ("minecraft:command_argument_type", command_argument_type::NAMES),
    ("minecraft:consume_effect_type", consume_effect_type::NAMES),
    ("minecraft:context_float_provider_type", context_float_provider_type::NAMES),
    ("minecraft:context_int_provider_type", context_int_provider_type::NAMES),
    ("minecraft:context_key_set", context_key_set::NAMES),
    ("minecraft:creative_mode_tab", creative_mode_tab::NAMES),
    ("minecraft:custom_stat", custom_stat::NAMES),
    ("minecraft:data_component_predicate_type", data_component_predicate_type::NAMES),
    ("minecraft:data_component_type", data_component_type::NAMES),
    ("minecraft:debug_subscription", debug_subscription::NAMES),
    ("minecraft:dialog_action_type", dialog_action_type::NAMES),
    ("minecraft:dialog_body_type", dialog_body_type::NAMES),
    ("minecraft:dialog_type", dialog_type::NAMES),
    ("minecraft:enchantment_effect_component_type", enchantment_effect_component_type::NAMES),
    ("minecraft:enchantment_entity_effect_type", enchantment_entity_effect_type::NAMES),
    ("minecraft:enchantment_level_based_value_type", enchantment_level_based_value_type::NAMES),
    ("minecraft:enchantment_location_based_effect_type", enchantment_location_based_effect_type::NAMES),
    ("minecraft:enchantment_provider_type", enchantment_provider_type::NAMES),
    ("minecraft:enchantment_value_effect_type", enchantment_value_effect_type::NAMES),
    ("minecraft:entity_sub_predicate_type", entity_sub_predicate_type::NAMES),
    ("minecraft:entity_type", entity_type::NAMES),
    ("minecraft:environment_attribute", environment_attribute::NAMES),
    ("minecraft:float_provider_type", float_provider_type::NAMES),
    ("minecraft:fluid", fluid::NAMES),
    ("minecraft:game_event", game_event::NAMES),
    ("minecraft:game_rule", game_rule::NAMES),
    ("minecraft:height_provider_type", height_provider_type::NAMES),
    ("minecraft:incoming_rpc_methods", incoming_rpc_methods::NAMES),
    ("minecraft:input_control_type", input_control_type::NAMES),
    ("minecraft:int_provider_type", int_provider_type::NAMES),
    ("minecraft:item", item::NAMES),
    ("minecraft:loot_condition_type", loot_condition_type::NAMES),
    ("minecraft:loot_function_type", loot_function_type::NAMES),
    ("minecraft:loot_nbt_provider_type", loot_nbt_provider_type::NAMES),
    ("minecraft:loot_pool_entry_type", loot_pool_entry_type::NAMES),
    ("minecraft:loot_score_provider_type", loot_score_provider_type::NAMES),
    ("minecraft:map_decoration_type", map_decoration_type::NAMES),
    ("minecraft:memory_module_type", memory_module_type::NAMES),
    ("minecraft:menu", menu::NAMES),
    ("minecraft:mob_effect", mob_effect::NAMES),
    ("minecraft:number_format_type", number_format_type::NAMES),
    ("minecraft:outgoing_rpc_methods", outgoing_rpc_methods::NAMES),
    ("minecraft:particle_type", particle_type::NAMES),
    ("minecraft:permission_check_type", permission_check_type::NAMES),
    ("minecraft:permission_type", permission_type::NAMES),
    ("minecraft:point_of_interest_type", point_of_interest_type::NAMES),
    ("minecraft:pos_rule_test", pos_rule_test::NAMES),
    ("minecraft:position_source_type", position_source_type::NAMES),
    ("minecraft:potion", potion::NAMES),
    ("minecraft:recipe_book_category", recipe_book_category::NAMES),
    ("minecraft:recipe_display", recipe_display::NAMES),
    ("minecraft:recipe_serializer", recipe_serializer::NAMES),
    ("minecraft:recipe_type", recipe_type::NAMES),
    ("minecraft:rule_block_entity_modifier", rule_block_entity_modifier::NAMES),
    ("minecraft:rule_test_type", rule_test_type::NAMES),
    ("minecraft:sensor_type", sensor_type::NAMES),
    ("minecraft:slot_display", slot_display::NAMES),
    ("minecraft:slot_source_type", slot_source_type::NAMES),
    ("minecraft:sound_event", sound_event::NAMES),
    ("minecraft:spawn_condition_type", spawn_condition_type::NAMES),
    ("minecraft:stat_type", stat_type::NAMES),
    ("minecraft:test_environment_definition_type", test_environment_definition_type::NAMES),
    ("minecraft:test_function", test_function::NAMES),
    ("minecraft:test_instance_type", test_instance_type::NAMES),
    ("minecraft:ticket_type", ticket_type::NAMES),
    ("minecraft:trigger_type", trigger_type::NAMES),
    ("minecraft:villager_profession", villager_profession::NAMES),
    ("minecraft:villager_type", villager_type::NAMES),
    ("minecraft:worldgen/biome_source", biome_source::NAMES),
    ("minecraft:worldgen/block_state_provider_type", block_state_provider_type::NAMES),
    ("minecraft:worldgen/carver_type", carver_type::NAMES),
    ("minecraft:worldgen/chunk_generator", chunk_generator::NAMES),
    ("minecraft:worldgen/density_function_type", density_function_type::NAMES),
    ("minecraft:worldgen/feature_size_type", feature_size_type::NAMES),
    ("minecraft:worldgen/feature_type", feature_type::NAMES),
    ("minecraft:worldgen/foliage_placer_type", foliage_placer_type::NAMES),
    ("minecraft:worldgen/material_condition_type", material_condition_type::NAMES),
    ("minecraft:worldgen/material_rule_type", material_rule_type::NAMES),
    ("minecraft:worldgen/placement_modifier_type", placement_modifier_type::NAMES),
    ("minecraft:worldgen/pool_alias_binding", pool_alias_binding::NAMES),
    ("minecraft:worldgen/root_placer_type", root_placer_type::NAMES),
    ("minecraft:worldgen/structure_piece", structure_piece::NAMES),
    ("minecraft:worldgen/structure_placement", structure_placement::NAMES),
    ("minecraft:worldgen/structure_pool_element", structure_pool_element::NAMES),
    ("minecraft:worldgen/structure_processor", structure_processor::NAMES),
    ("minecraft:worldgen/structure_type", structure_type::NAMES),
    ("minecraft:worldgen/tree_decorator_type", tree_decorator_type::NAMES),
    ("minecraft:worldgen/trunk_placer_type", trunk_placer_type::NAMES),
];
