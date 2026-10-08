#![allow(dead_code)]

use std::sync::LazyLock;

use bevy_app::App;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, TagKey, rl};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_protocol::item::{Tool, ToolRule};
use mcrs_minecraft_registry::{HolderSet, Id, LoadReport, RegistrySet};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registries::{
    insert_registry_resources, share_registries, static_registries, test_registries,
};
use mcrs_minecraft_world::resolvers::run_resolvers;
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
    insert_registry_resources(app.world_mut(), test_registries());
    share_registries(app.world_mut());
    app.add_plugins((
        mcrs_minecraft_worldgen_generator::ids::GeneratorIdsPlugin,
        mcrs_minecraft_inventory::InventoryIdsPlugin,
    ));
    run_resolvers(app.world_mut(), test_registries()).unwrap_or_else(|report| panic!("{report}"));
}

pub fn dimension_type(name: &str) -> Id<DimensionType> {
    test_registries()
        .registry::<DimensionType>()
        .and_then(|registry| registry.by_name(name))
        .unwrap_or_else(|| panic!("the dimension type {name} is loaded"))
}

pub fn registry_set() -> &'static RegistrySet {
    static REGISTRIES: LazyLock<RegistrySet> =
        LazyLock::new(|| static_registries().unwrap_or_else(|report| panic!("{report}")));
    &REGISTRIES
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

/// A tool whose one rule names `#minecraft:mineable/pickaxe`, so the component carries a tag.
pub fn pickaxe_tagged_tool(registries: &RegistrySet) -> Tool {
    let pickaxe = registries
        .tags::<Block>()
        .unwrap()
        .get(&TagKey::<Block, _>::from_location(
            rl!("minecraft:mineable/pickaxe").to_arc(),
        ))
        .unwrap();
    Tool {
        rules: vec![ToolRule {
            blocks: HolderSet::Named(pickaxe),
            speed: Some(2.0),
            correct_for_drops: None,
        }],
        default_mining_speed: 1.0,
        damage_per_block: Bounded(1),
        can_destroy_blocks_in_creative: true,
    }
}
