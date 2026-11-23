#![recursion_limit = "256"]

mod dimension_plans;
mod dimension_world;
mod generated_ticks;
mod generation_dispatch;
mod generation_input;
mod generation_stage;
#[cfg(test)]
mod generation_tests;
mod saved_monuments;
mod template_asset;
mod template_handle;
mod world_gen_plugin;
mod world_generation;
mod worldgen_registries_plugin;
mod worldgen_tables;

pub use dimension_plans::DimensionPlans;
pub use template_asset::{TemplateAsset, WorldgenAssetsPlugin, pool_templates};
pub use template_handle::template_handle;
pub use world_gen_plugin::WorldGenPlugin;
pub use world_generation::WorldGeneration;
pub use worldgen_registries_plugin::WorldgenRegistriesPlugin;
pub use worldgen_tables::WorldgenTables;
