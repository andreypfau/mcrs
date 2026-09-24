use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use mcrs_minecraft_assets::asset::JsonLoader;

use mcrs_minecraft_biome::Biome;

pub struct BetaBiomeSourcePlugin;

impl Plugin for BetaBiomeSourcePlugin {
    fn build(&self, app: &mut App) {
        // The per-dim sub-app has its own AssetServer that never sees
        // MinecraftWorldPlugin's registrations, so Biome must be registered here.
        app.init_asset::<Biome>();
        app.register_asset_loader(JsonLoader::<Biome>::default());
    }
}
