use std::sync::Arc;

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::Resource;
use bevy_reflect::TypePath;

use crate::biome::source::BiomeSource;
use crate::biome::{Biome, BiomeLoader};

/// Carries the active world preset's Beta biome source.
///
/// Present only when the active preset uses `mcrs:beta` as its overworld biome source.
/// `dispatch_column_generation` reads this resource to fill `BiomePalette` from climate.
#[derive(Resource, TypePath, Clone)]
pub struct ActiveBiomeSource(pub Arc<BiomeSource>);

pub struct BetaBiomeSourcePlugin;

impl Plugin for BetaBiomeSourcePlugin {
    fn build(&self, app: &mut App) {
        // The per-dim sub-app has its own AssetServer that never sees
        // MinecraftWorldPlugin's registrations, so Biome must be registered here.
        app.init_asset::<Biome>();
        app.register_asset_loader(BiomeLoader);
    }
}
