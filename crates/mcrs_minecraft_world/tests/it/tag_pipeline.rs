use bevy_app::App;
use mcrs_minecraft_assets::tag::TagLoader;
use mcrs_minecraft_assets::tag::registry::{DynTagRegistry, TagRegistry};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::tags as block_tags;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_entity::EntityType;
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::{Id, RegistrySet};

use crate::common::workspace_root;

pub fn tags_load_resolve_and_freeze_on_the_way_to_playing(app: &App) {
    assert!(
        app.world()
            .get_resource::<TagLoader<Block, u32>>()
            .is_none(),
        "the loader must be consumed by the freeze"
    );

    let tags = app.world().resource::<DynTagRegistry<Block>>();
    let blocks = app.world().resource::<Blocks>();
    let index = |name: &str| blocks.index_of(name).expect("the corpus declares it");

    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, index("minecraft:stone")));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, index("minecraft:dirt")));

    // `#minecraft:planks` is reached only through nested `#tag` entries of
    // `#minecraft:mineable/axe`.
    assert!(tags.contains(&block_tags::MINEABLE_AXE, index("minecraft:oak_planks")));

    // A block no static registry ever named still lands in its tags.
    assert!(tags.contains(
        &block_tags::MINEABLE_PICKAXE,
        index("minecraft:polished_tuff_stairs")
    ));
    assert!(tags.contains(
        &block_tags::MINEABLE_AXE,
        index("minecraft:mangrove_trapdoor")
    ));
    assert!(tags.contains(&block_tags::WOOL, index("minecraft:magenta_wool")));

    // And a tag no Rust constant names is there, because the pack ships it.
    let stairs =
        TagKey::<Block, _>::from_location(ResourceLocation::parse("minecraft:stairs").unwrap());
    assert!(tags.contains(&stairs, index("minecraft:polished_tuff_stairs")));
    assert!(!tags.contains(&stairs, index("minecraft:stone")));

    let resolved = tags.iter().count();
    println!("block tags resolved: {resolved}");
    let shipped = count_json(&workspace_root().join("assets/minecraft/tags/block"));
    assert!(
        resolved >= shipped,
        "the loader must pick up every shipped block tag: {resolved} < {shipped}"
    );
}

pub fn entity_type_tags_are_numbered_by_the_report(app: &App) {
    let registry = app
        .world()
        .resource::<RegistrySet>()
        .registry::<EntityType>()
        .expect("the report carries the entity types");
    let tags = app
        .world()
        .resource::<TagRegistry<EntityType, Id<EntityType>>>();
    let skeletons = TagKey::<EntityType, _>::from_location(
        ResourceLocation::parse("minecraft:skeletons").unwrap(),
    );

    let members: Vec<usize> = tags
        .get(&skeletons)
        .expect("the pack ships the skeletons tag")
        .iter()
        .map(Id::index)
        .collect();
    let mut expected: Vec<usize> = [
        "minecraft:skeleton",
        "minecraft:stray",
        "minecraft:wither_skeleton",
        "minecraft:skeleton_horse",
        "minecraft:bogged",
        "minecraft:parched",
    ]
    .into_iter()
    .map(|name| registry.require(name).unwrap().index())
    .collect();
    expected.sort_unstable();
    assert_eq!(members, expected);
}

fn count_json(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            if path.is_dir() {
                count_json(&path)
            } else {
                usize::from(path.extension().is_some_and(|e| e == "json"))
            }
        })
        .sum()
}
