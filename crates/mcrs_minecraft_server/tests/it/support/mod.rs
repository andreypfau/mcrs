#![allow(dead_code)]

use bevy_app::App;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::{Items, test_corpus};

/// A dimension sub-app is handed the real corpus at spawn, and worldgen
/// resolves the block it fills terrain with against it, so a stub would only
/// move the failure somewhere less obvious.
pub fn standalone_corpus() -> &'static (Blocks, Items) {
    test_corpus()
}

pub fn insert_corpus(app: &mut App) {
    let (blocks, items) = standalone_corpus();
    app.insert_resource(blocks.clone());
    app.insert_resource(items.clone());
}
