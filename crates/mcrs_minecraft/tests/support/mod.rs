#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_app::App;
use bevy_asset::AssetServer;
use mcrs_vanilla::block::definition::{Blocks, load_block_definitions};

/// The real corpus, read once per test binary. A dimension sub-app is handed
/// this at spawn, and worldgen resolves the block it fills terrain with against
/// it, so a stub would only move the failure somewhere less obvious.
pub fn corpus(app: &App) -> Blocks {
    static CORPUS: OnceLock<Blocks> = OnceLock::new();
    CORPUS
        .get_or_init(|| {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let (definitions, _) =
                load_block_definitions(&asset_server).expect("the block definition corpus loads");
            Blocks(Arc::new(definitions))
        })
        .clone()
}
