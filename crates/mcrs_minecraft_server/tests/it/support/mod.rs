use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registries::insert_registry_resources;
use mcrs_minecraft_world::registry_load::{test_registries, test_registries_arc};
use mcrs_minecraft_world::registry_snapshot::install_registries;
use mcrs_minecraft_world::resolvers::{AddRegistryResolver, run_resolvers};
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

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
    app.add_message::<mcrs_minecraft_registries::RegistriesReloaded>();
    insert_registry_resources(app.world_mut(), test_registries());
    app.add_registry_resolver(mcrs_minecraft_world::registries::clock_time_markers);
    app.add_plugins(mcrs_minecraft_worldgen_generator::ids::GeneratorIdsPlugin);
    let resolved = run_resolvers(app.world_mut(), test_registries_arc())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics}"));
    install_registries(app.world_mut(), Arc::clone(test_registries_arc()), resolved);
}

pub fn dimension_type(name: &str) -> Id<DimensionType> {
    test_registries()
        .registry::<DimensionType>()
        .and_then(|registry| registry.by_name(name))
        .unwrap_or_else(|| panic!("the dimension type {name} is loaded"))
}

pub fn preset(name: &str) -> Dimensions {
    let set = test_registries();
    let id = set
        .registry::<WorldPreset>()
        .and_then(|registry| registry.by_name(name))
        .unwrap_or_else(|| panic!("the preset {name} is loaded"));
    set.entries::<WorldPreset, WorldPreset>().unwrap()[id]
        .dimensions
        .clone()
}

pub fn baked(
    dimensions: &Dimensions,
    set: &RegistrySet,
) -> Vec<(ResourceKey<Dimension>, DimensionEntry)> {
    let mut report = LoadReport::new();
    let list = bake(dimensions, set, &mut report);
    assert!(report.is_empty(), "{report}");
    list.expect("a list with an overworld bakes")
}

/// The vanilla preset's three dimensions, then a data-pack dimension, baked as the server bakes them.
pub fn dimension_list_with_extra() -> mcrs_minecraft_server::world_options::DimensionList {
    use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

    let set = test_registries();
    let mut dimensions = preset(mcrs_minecraft_world::keys::world_preset::NORMAL.as_str());
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
    mcrs_minecraft_server::world_options::DimensionList::new(baked(&dimensions, set))
}

pub fn update_until_playing(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(45);
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }
}
