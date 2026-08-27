mod bench_columns;
mod beta_biome_palette;
mod beta_cave_parity;
mod beta_ore_distribution;
mod beta_surface;
mod beta_surface_parity;
mod cell_fill;

use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_world::block::definition::{BlockDefinitions, load_block_definitions};

/// The corpus, loaded once per test binary. Worldgen resolves every block it
/// places against it, so a stub would fail at the first lookup.
pub fn corpus() -> &'static BlockDefinitions {
    static CORPUS: OnceLock<BlockDefinitions> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        load_block_definitions(&asset_server)
            .expect("the corpus loads")
            .0
    })
}
