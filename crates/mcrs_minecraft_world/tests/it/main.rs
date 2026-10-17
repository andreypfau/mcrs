#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON, and the test finds the reference sources through the environment"
)]

mod biome_tags;
mod block_definition_corpus;
mod common;
mod data_pack_stack;
mod dimension_list;
mod dimension_router_set;
mod from_network;
mod item_corpus;
mod loaded_registries;
mod player_dat;
mod playing;
mod registry_values;
mod resolvers;
mod sound_sets;
mod structure_assets;
mod synced_registries;
mod tag_pipeline;
mod timeline_pipeline;
mod tool_rules;
mod world_registry_ids;
