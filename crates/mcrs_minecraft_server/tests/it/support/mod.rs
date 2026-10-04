#![allow(dead_code)]

use std::sync::LazyLock;

use bevy_app::App;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registries::static_registries;

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

pub fn registry_set() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> = LazyLock::new(|| {
        let report = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .expect("the registries report is readable");
        static_registries(&report).unwrap_or_else(|report| panic!("{report}"))
    });
    &SET
}
