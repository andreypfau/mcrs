#![allow(dead_code)]

use std::sync::LazyLock;

use bevy_app::App;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_assets::snapshot::RegistrySnapshot;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::{Item, Items};
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::shared::share;
use mcrs_minecraft_registry::{Entries, Registry, RegistrySet};
use mcrs_minecraft_world::entity::minecraft::EntityIds;
use mcrs_minecraft_world::item::{test_corpus, test_enchantment_registry, test_enchantments};
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

pub fn insert_registries(app: &mut App) {
    insert_corpus(app);
    app.insert_resource(registry_set().clone());
    app.insert_resource(entity_ids().clone());
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(test_enchantment_registry());
    app.insert_resource(test_enchantments());
    app.insert_resource(DynTagRegistry::<Block>::default());
    app.insert_resource(DynTagRegistry::<Item>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());
    let world = app.world_mut();
    share::<RegistrySet>(world);
    share::<EntityIds>(world);
    share::<RegistryAccess>(world);
    share::<Blocks>(world);
    share::<Items>(world);
    share::<Registry<EnchantmentData>>(world);
    share::<Entries<EnchantmentData, EnchantmentData>>(world);
    share::<DynTagRegistry<Block>>(world);
    share::<DynTagRegistry<Item>>(world);
    share::<RegistrySnapshot<Biome>>(world);
}

fn registries() -> &'static (RegistrySet, EntityIds) {
    static REGISTRIES: LazyLock<(RegistrySet, EntityIds)> = LazyLock::new(|| {
        let report = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .expect("the registries report is readable");
        static_registries(&report).unwrap_or_else(|report| panic!("{report}"))
    });
    &REGISTRIES
}

pub fn registry_set() -> &'static RegistrySet {
    &registries().0
}

pub fn entity_ids() -> &'static EntityIds {
    &registries().1
}
