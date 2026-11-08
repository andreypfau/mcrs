// std: bevy_ecs

mod dimension_world;
mod features;
mod flat_world;
mod generated_ticks;
mod generation_dispatch;
mod generation_input;
mod generation_stage;
#[cfg(test)]
mod generation_tests;
mod modern_carvers;
mod routers;
mod structures;
mod template_asset;
mod template_handle;
mod world_gen_plugin;
mod world_generation;
mod worldgen_registries_plugin;

pub use routers::{DimensionRouter, DimensionRouters};
pub use template_asset::{TemplateAsset, WorldgenAssetsPlugin, pool_templates};
pub use template_handle::template_handle;
pub use world_gen_plugin::WorldGenPlugin;
pub use world_generation::WorldGeneration;
pub use worldgen_registries_plugin::WorldgenRegistriesPlugin;
