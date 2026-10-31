use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_engine::client_info::ClientInfo;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_protocol::setting::ChatMode;
use mcrs_minecraft_registries::{Registries, Resolver};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registry_load::test_registries;
use mcrs_minecraft_world::registry_snapshot::install_registries;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

/// A dimension sub-app is handed the real corpus at spawn, and worldgen
/// resolves the block it fills terrain with against it, so a stub would only
/// move the failure somewhere less obvious.
pub fn standalone_corpus() -> &'static (Blocks, Items) {
    test_corpus()
}

/// The host holds the loaded set, dimension types included, because a
/// dimension is spawned with the type its entry names.
pub fn insert_registries(app: &mut App) {
    let (blocks, items) = standalone_corpus();
    install_registries(
        app.world_mut(),
        test_registries().clone(),
        blocks.clone(),
        items.clone(),
    );
}

pub fn dimension_type(name: &str) -> Id<DimensionType> {
    Resolver::id(
        test_registries(),
        &ResourceKey::from_location(ResourceLocation::read(name).unwrap()),
    )
    .unwrap_or_else(|error| panic!("the dimension type {name} is loaded: {error}"))
}

pub fn preset(name: &str) -> Dimensions {
    let presets = test_registries()
        .registry::<WorldPreset>()
        .expect("the presets are loaded");
    presets
        .by_name(&ResourceLocation::read(name).unwrap())
        .and_then(|id| presets.get(id))
        .unwrap_or_else(|| panic!("the preset {name} is loaded"))
        .dimensions
        .clone()
}

pub fn baked(
    dimensions: &Dimensions,
    registries: &Registries,
) -> Vec<(ResourceKey<Dimension>, DimensionEntry)> {
    bake(dimensions, registries).expect("a list with an overworld bakes")
}

/// The vanilla preset's three dimensions, then a data-pack dimension, baked as the server bakes them.
pub fn dimension_list_with_extra() -> DimensionList {
    use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

    let mut dimensions = preset(mcrs_minecraft_world::keys::world_preset::NORMAL.as_str());
    dimensions.insert(
        ResourceKey::from_location("test:extra".parse().unwrap()),
        DimensionEntry {
            dimension_type: dimension_type(
                mcrs_minecraft_dimension::keys::dimension_type::OVERWORLD.as_str(),
            ),
            generator: ChunkGenerator::Debug,
        },
    );
    DimensionList::new(
        baked(&dimensions, test_registries())
            .into_iter()
            .map(|(key, entry)| (key, entry.dimension_type)),
    )
}

pub fn update_until_playing(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(45);
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }
}

pub fn client_info_with_view_distance(view_distance: u8) -> ClientInfo {
    ClientInfo {
        locale: "en_us".to_owned(),
        view_distance,
        chat_mode: ChatMode::Enabled,
    }
}
