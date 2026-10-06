#![allow(dead_code)]

use std::sync::LazyLock;

use bevy_app::App;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::{Id, RegistrySet};
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registries::{
    insert_registry_resources, share_registries, static_registries, test_registries,
};
use mcrs_minecraft_world::resolvers::run_resolvers;
use mcrs_minecraft_worldgen::tables::WorldgenTables;

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

/// The host holds the loaded set, dimension types included, because a
/// dimension is spawned with the type its entry names.
pub fn insert_registries(app: &mut App) {
    insert_corpus(app);
    insert_registry_resources(app.world_mut(), test_registries());
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(WorldgenTables::default());
    share_registries(app.world_mut());
    app.add_plugins((
        mcrs_minecraft_worldgen_generator::ids::GeneratorIdsPlugin,
        mcrs_minecraft_inventory::InventoryIdsPlugin,
    ));
    run_resolvers(app.world_mut(), test_registries()).unwrap_or_else(|report| panic!("{report}"));
}

fn registries() -> &'static RegistrySet {
    static REGISTRIES: LazyLock<RegistrySet> =
        LazyLock::new(|| static_registries().unwrap_or_else(|report| panic!("{report}")));
    &REGISTRIES
}

pub fn dimension_type(name: &str) -> Id<DimensionType> {
    test_registries()
        .registry::<DimensionType>()
        .and_then(|registry| registry.by_name(name))
        .unwrap_or_else(|| panic!("the dimension type {name} is loaded"))
}

pub fn registry_set() -> &'static RegistrySet {
    registries()
}

/// The vanilla preset's three dimensions, then a data-pack dimension, baked as the server bakes them.
pub fn dimension_list_with_extra() -> mcrs_minecraft_server::world_options::DimensionList {
    use mcrs_minecraft_core::ResourceKey;
    use mcrs_minecraft_registry::LoadReport;
    use mcrs_minecraft_world::dimension::{DimensionEntry, bake};
    use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
    use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

    let set = test_registries();
    let preset = set
        .registry::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset>()
        .and_then(|registry| registry.get(&mcrs_minecraft_world::keys::world_preset::NORMAL))
        .expect("the normal preset is loaded");
    let mut dimensions = set
        .entries::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset, WorldPreset>()
        .unwrap()[preset]
        .dimensions
        .clone();
    dimensions.insert(
        ResourceKey::from_location("test:extra".parse().unwrap()),
        DimensionEntry {
            dimension_type: set
                .registry::<DimensionType>()
                .and_then(|registry| {
                    registry.get(&mcrs_minecraft_dimension::keys::dimension_type::OVERWORLD)
                })
                .expect("the dimension type is loaded"),
            generator: ChunkGenerator::Debug,
        },
    );
    let mut report = LoadReport::new();
    let list = bake(&dimensions, set, &mut report);
    assert!(report.is_empty(), "{report}");
    mcrs_minecraft_server::world_options::DimensionList::new(list.expect("the list bakes"))
}
