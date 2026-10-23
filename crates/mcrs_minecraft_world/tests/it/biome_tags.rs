use bevy_app::App;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_world::registry_load::sent_by;

use crate::common::tag_members;

fn members(app: &App, tag: &str) -> Vec<String> {
    let mut names = tag_members::<Biome>(app.world().resource::<CurrentRegistries>(), tag);
    names.sort();
    names
}

pub fn the_shipped_biome_tags_resolve(app: &App) {
    assert_eq!(
        members(app, "minecraft:has_structure/village_plains"),
        ["minecraft:meadow", "minecraft:plains"]
    );

    let biased = members(app, "minecraft:stronghold_biased_to");
    assert_eq!(biased.len(), 38);
    assert!(biased.contains(&"minecraft:plains".to_owned()));

    let nested = members(app, "minecraft:has_structure/stronghold");
    assert_eq!(nested, members(app, "minecraft:is_overworld"));
    assert_eq!(nested.len(), 57);
}

pub fn the_biome_registry_and_the_synced_registry_agree_on_the_id_space(app: &App) {
    let set = app.world().resource::<CurrentRegistries>();
    let biomes = set
        .names_of(&ResourceLocation::read("minecraft:worldgen/biome").unwrap())
        .expect("the loaded set holds the biomes");
    let (_, sent) = sent_by(set, false)
        .registries
        .into_iter()
        .find(|(registry, _)| registry.as_str() == "minecraft:worldgen/biome")
        .expect("the biomes are synced");

    assert!(!biomes.is_empty());
    let sent: Vec<&ResourceLocation> = sent.iter().map(|(name, _)| name).collect();
    assert_eq!(biomes.iter().collect::<Vec<_>>(), sent);
}
