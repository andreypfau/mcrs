use bevy_app::App;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_registries::{CurrentRegistries, IdSet};
use mcrs_minecraft_resource::{ResourceLocationBuf, TagKey, TagKeyBuf};

use crate::common::{read_named, tag_members};

pub fn tags_load_resolve_and_freeze_on_the_way_to_playing(app: &App) {
    let tags = app
        .world()
        .resource::<CurrentRegistries>()
        .tags::<Block>()
        .expect("the load builds the block tags");
    let blocks = app.world().resource::<Blocks>();
    let contains = |tag: &TagKey<'static, Block>, name: &str| {
        let id = blocks.id_of(name).expect("the corpus declares it");
        tags.contains(tags.id(tag).expect("the pack ships the tag"), id)
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
        .id(&TagKeyBuf::from_location(
            ResourceLocationBuf::read("minecraft:stairs").unwrap(),
        ))
        .expect("the pack ships the stairs tag");
    assert!(tags.contains(
        stairs,
        blocks.id_of("minecraft:polished_tuff_stairs").unwrap()
    ));
    assert!(!tags.contains(stairs, blocks.id_of("minecraft:stone").unwrap()));

    let resolved = tags.iter().count();
    println!("block tags resolved: {resolved}");
    let shipped = count_json(&mcrs_minecraft_fixtures::vanilla_dir().join("tags/block"));
    assert!(
        resolved >= shipped,
        "the loader must pick up every shipped block tag: {resolved} < {shipped}"
    );
}

pub fn entity_type_tags_are_numbered_by_the_report(app: &App) {
    let set = app.world().resource::<CurrentRegistries>();
    let registry = set
        .entries::<EntityType>()
        .expect("the report carries the entity types");
    let tags = set
        .tags::<EntityType>()
        .expect("the load builds the entity type tags");
    let skeletons = ResourceLocationBuf::read("minecraft:skeletons").unwrap();

    let mut members: Vec<usize> = tags
        .members(
            tags.id(&TagKeyBuf::from_location(skeletons))
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
    .map(|name| {
        registry
            .by_name(&ResourceLocationBuf::read(name).unwrap())
            .unwrap()
            .index()
    })
    .collect();
    expected.sort_unstable();
    assert_eq!(members, expected);
}

#[test]
fn the_loaded_set_holds_ordered_tags_of_every_registry() {
    assert_eq!(
        tag_members::<Block>(test_registries(), "minecraft:mineable/pickaxe")[..4],
        [
            "minecraft:stone",
            "minecraft:granite",
            "minecraft:polished_granite",
            "minecraft:diorite"
        ]
    );
    assert_eq!(
        tag_members::<Timeline>(test_registries(), "minecraft:in_overworld"),
        [
            "minecraft:villager_schedule",
            "minecraft:day",
            "minecraft:moon",
            "minecraft:early_game"
        ]
    );
    assert_eq!(
        tag_members::<Biome>(test_registries(), "minecraft:is_savanna"),
        [
            "minecraft:savanna",
            "minecraft:savanna_plateau",
            "minecraft:windswept_savanna"
        ]
    );
}

#[test]
fn a_block_tag_named_by_a_predicate_resolves_to_its_members() {
    let set = test_registries();
    let stones: IdSet<Block> = read_named(set, r##""#minecraft:base_stone_overworld""##).unwrap();
    let tags = set.tags::<Block>().unwrap();
    assert!(stones.contains(&tags, Block::Stone.id()));
    assert!(!stones.contains(&tags, Block::Dirt.id()));
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
