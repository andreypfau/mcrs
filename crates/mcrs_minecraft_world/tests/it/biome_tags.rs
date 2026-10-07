use bevy_app::App;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_registry::{Registry, RegistrySet};

use crate::common::tag_members;

fn members(app: &App, tag: &str) -> Vec<String> {
    let mut names = tag_members::<Biome>(app.world().resource::<RegistrySet>(), tag);
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
    assert_eq!(nested.len(), 56);
}

pub fn the_biome_registry_and_the_synced_registry_agree_on_the_id_space(app: &App) {
    let biomes = app.world().resource::<Registry<Biome>>();
    let (table, column) = app
        .world()
        .resource::<RegistrySet>()
        .synced()
        .find(|(table, _)| table.registry().as_str() == "minecraft:worldgen/biome")
        .expect("the biomes are synced");

    assert_eq!(biomes.len(), table.len());
    assert_eq!(biomes.len(), column.len());
    assert!(!biomes.is_empty());
    for (id, name) in biomes.ids().zip(table.names()) {
        assert_eq!(
            biomes.name(id).map(|name| name.as_str()),
            Some(name.as_str())
        );
    }
}
