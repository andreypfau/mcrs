#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON"
)]

mod biome_tags;
mod block_definition_corpus;
mod broken_packs;
mod common;
mod data_pack_stack;
mod dimension_list;
mod dimension_router_set;
mod item_corpus;
mod loaded_registries;
mod playing;
mod received;
mod registry_values;
mod sound_sets;
mod structure_assets;
mod tag_pipeline;
mod timeline_pipeline;
mod world_registry_ids;
