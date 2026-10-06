use bevy_app::App;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_keys::block_tags;
use mcrs_minecraft_keys::{Block, EntityType};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::registries::test_registries;

use crate::common::workspace_root;

pub fn tags_load_resolve_and_freeze_on_the_way_to_playing(app: &App) {
    let tags = app
        .world()
        .resource::<RegistrySet>()
        .tags::<Block>()
        .expect("the load builds the block tags");
    let blocks = app.world().resource::<Blocks>();
    let contains = |tag: &TagKey<Block, &'static str>, name: &str| {
        let id = blocks.id_of(name).expect("the corpus declares it");
        tags.contains(tags.get(tag).expect("the pack ships the tag"), id)
    };

    assert!(contains(&block_tags::MINEABLE_PICKAXE, "minecraft:stone"));
    assert!(!contains(&block_tags::MINEABLE_PICKAXE, "minecraft:dirt"));

    // `#minecraft:planks` is reached only through nested `#tag` entries of
    // `#minecraft:mineable/axe`.
    assert!(contains(&block_tags::MINEABLE_AXE, "minecraft:oak_planks"));

    // A block no static registry ever named still lands in its tags.
    assert!(contains(
        &block_tags::MINEABLE_PICKAXE,
        "minecraft:polished_tuff_stairs"
    ));
    assert!(contains(
        &block_tags::MINEABLE_AXE,
        "minecraft:mangrove_trapdoor"
    ));
    assert!(contains(&block_tags::WOOL, "minecraft:magenta_wool"));

    // And a tag no Rust constant names is there, because the pack ships it.
    let stairs = tags
        .get(&TagKey::<Block, _>::from_location(
            ResourceLocation::read("minecraft:stairs").unwrap(),
        ))
        .expect("the pack ships the stairs tag");
    assert!(tags.contains(
        stairs,
        blocks.id_of("minecraft:polished_tuff_stairs").unwrap()
    ));
    assert!(!tags.contains(stairs, blocks.id_of("minecraft:stone").unwrap()));

    let resolved = tags.table().len();
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
        .resource::<RegistrySet>()
        .tags::<EntityType>()
        .expect("the load builds the entity type tags");
    let skeletons = TagKey::<EntityType, _>::from_location(
        ResourceLocation::read("minecraft:skeletons").unwrap(),
    );

    let mut members: Vec<usize> = tags
        .members(
            tags.get(&skeletons)
                .expect("the pack ships the skeletons tag"),
        )
        .map(|id| id.index())
        .collect();
    members.sort_unstable();
    let mut expected: Vec<usize> = [
        "minecraft:skeleton",
        "minecraft:stray",
        "minecraft:wither_skeleton",
        "minecraft:skeleton_horse",
        "minecraft:bogged",
        "minecraft:parched",
    ]
    .into_iter()
    .map(|name| registry.require_by_name(name).unwrap().index())
    .collect();
    expected.sort_unstable();
    assert_eq!(members, expected);
}

fn loaded_members<R: mcrs_minecraft_registry::Registered>(tag: &str) -> Vec<String> {
    let set = test_registries();
    let registry = set.registry::<R>().expect("the registry is loaded");
    let tags = set
        .tags::<R>()
        .unwrap_or_else(|| panic!("the load builds the tags of {}", R::REGISTRY));
    let key = TagKey::<R, _>::from_location(ResourceLocation::read(tag).unwrap());
    let id = tags
        .get(&key)
        .unwrap_or_else(|| panic!("{tag} is a loaded tag of {}", R::REGISTRY));
    tags.members(id)
        .map(|member| registry.name(member).unwrap().as_str().to_owned())
        .collect()
}

#[test]
fn the_loaded_set_holds_ordered_tags_of_every_registry() {
    assert_eq!(
        loaded_members::<Block>("minecraft:mineable/pickaxe")[..4],
        [
            "minecraft:stone",
            "minecraft:granite",
            "minecraft:polished_granite",
            "minecraft:diorite"
        ]
    );
    assert_eq!(
        loaded_members::<Timeline>("minecraft:in_overworld"),
        [
            "minecraft:villager_schedule",
            "minecraft:day",
            "minecraft:moon",
            "minecraft:early_game"
        ]
    );
    assert_eq!(
        loaded_members::<Biome>("minecraft:is_savanna"),
        [
            "minecraft:savanna",
            "minecraft:savanna_plateau",
            "minecraft:windswept_savanna"
        ]
    );
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
