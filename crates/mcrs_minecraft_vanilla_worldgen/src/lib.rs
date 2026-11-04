// std: bevy_ecs

mod chunk_plugin;
mod dimension_world;
mod features;
mod generation_dispatch;
mod generation_input;
mod generation_link;
mod generation_output;
mod generation_stage;
#[cfg(test)]
mod generation_tests;
mod modern_carvers;
mod routers;
mod structures;
mod template_asset;
mod template_handle;
mod world_gen_plugin;
mod worldgen_registries_plugin;

pub use generation_link::GenerationLink;
pub use routers::{DimensionRouter, DimensionRouters};
pub use template_asset::{TemplateAsset, WorldgenAssetsPlugin, pool_templates};
pub use template_handle::template_handle;
pub use world_gen_plugin::WorldGenPlugin;
pub use worldgen_registries_plugin::WorldgenRegistriesPlugin;
