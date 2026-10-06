// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

#[rustfmt::skip]
pub mod attribute;
#[rustfmt::skip]
pub mod attribute_type;
#[rustfmt::skip]
pub mod block;
#[rustfmt::skip]
pub mod block_entity_type;
#[rustfmt::skip]
pub mod block_tags;
#[rustfmt::skip]
pub mod chunk_generator;
#[rustfmt::skip]
pub mod command_argument_type;
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
pub mod debug_subscription;
#[rustfmt::skip]
pub mod density_function;
#[rustfmt::skip]
pub mod dialog_action_type;
#[rustfmt::skip]
pub mod enchantment_provider_type;
#[rustfmt::skip]
pub mod entity_sub_predicate_type;
#[rustfmt::skip]
pub mod entity_type;
#[rustfmt::skip]
pub mod entity_type_tags;
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
pub mod int_provider_type;
#[rustfmt::skip]
pub mod item;
#[rustfmt::skip]
pub mod item_tags;
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
pub mod material_condition;
#[rustfmt::skip]
pub mod memory_module_type;
#[rustfmt::skip]
pub mod menu;
#[rustfmt::skip]
pub mod number_format_type;
#[rustfmt::skip]
pub mod outgoing_rpc_methods;
#[rustfmt::skip]
pub mod permission_check_type;
#[rustfmt::skip]
pub mod permission_type;
#[rustfmt::skip]
pub mod point_of_interest_type;
#[rustfmt::skip]
pub mod point_of_interest_type_tags;
#[rustfmt::skip]
pub mod predicate;
#[rustfmt::skip]
pub mod recipe_book_category;
#[rustfmt::skip]
pub mod recipe_serializer;
#[rustfmt::skip]
pub mod recipe_type;
#[rustfmt::skip]
pub mod registry;
#[rustfmt::skip]
pub mod sensor_type;
#[rustfmt::skip]
pub mod slot_source_type;
#[rustfmt::skip]
pub mod stat_type;
#[rustfmt::skip]
pub mod test_environment_definition_type;
#[rustfmt::skip]
pub mod test_function;
#[rustfmt::skip]
pub mod test_instance_type;
#[rustfmt::skip]
pub mod ticket_type;
#[rustfmt::skip]
pub mod trial_spawner;
#[rustfmt::skip]
pub mod trigger_type;

pub use registry::*;
