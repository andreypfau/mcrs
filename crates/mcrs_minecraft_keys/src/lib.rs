// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

#[rustfmt::skip]
pub mod activity;
#[rustfmt::skip]
pub mod attribute;
#[rustfmt::skip]
pub mod attribute_type;
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
pub mod carver;
#[rustfmt::skip]
pub mod carver_type;
#[rustfmt::skip]
pub mod chat_type;
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
pub mod creative_mode_tab;
#[rustfmt::skip]
pub mod custom_stat;
#[rustfmt::skip]
pub mod data_component_predicate_type;
#[rustfmt::skip]
pub mod data_component_type;
#[rustfmt::skip]
pub mod debug_subscription;
#[rustfmt::skip]
pub mod density_function;
#[rustfmt::skip]
pub mod density_function_type;
#[rustfmt::skip]
pub mod dialog_action_type;
#[rustfmt::skip]
pub mod dialog_body_type;
#[rustfmt::skip]
pub mod dialog_type;
#[rustfmt::skip]
pub mod dimension;
#[rustfmt::skip]
pub mod dimension_type;
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
pub mod int_provider_type;
#[rustfmt::skip]
pub mod item;
#[rustfmt::skip]
pub mod item_tags;
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
pub mod particle_type;
#[rustfmt::skip]
pub mod permission_check_type;
#[rustfmt::skip]
pub mod permission_type;
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
pub mod trade_set;
#[rustfmt::skip]
pub mod tree_decorator_type;
#[rustfmt::skip]
pub mod trial_spawner;
#[rustfmt::skip]
pub mod trigger_type;
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
pub mod world_preset;
#[rustfmt::skip]
pub mod world_preset_tags;

pub use registry::*;
