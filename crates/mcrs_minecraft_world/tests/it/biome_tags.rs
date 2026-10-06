use bevy_app::App;
use mcrs_minecraft_assets::RegistryAccess;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_registry::{Registry, RegistrySet};

fn members(app: &App, tag: &str) -> Vec<String> {
    let tags = app
        .world()
        .resource::<RegistrySet>()
        .tags::<Biome>()
        .expect("the load builds the biome tags");
    let biomes = app.world().resource::<Registry<Biome>>();
    let key = TagKey::<Biome, _>::from_location(ResourceLocation::read(tag).unwrap());
    let mut names: Vec<String> = tags
        .members(tags.get(&key).expect("the tag is resolved"))
        .map(|id| {
            biomes
                .name(id)
                .expect("a member id maps back")
                .as_str()
                .to_owned()
        })
        .collect();
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
    let synced = app
        .world()
        .resource::<RegistryAccess>()
        .iter()
        .find(|registry| registry.registry_key() == "minecraft:worldgen/biome")
        .expect("the biomes are synced");

    assert_eq!(biomes.len(), synced.len());
    assert!(!biomes.is_empty());
    for (id, entry) in biomes.ids().zip(synced.iter_entries()) {
        assert_eq!(
            biomes.name(id).map(|name| name.as_str()),
            Some(entry.location.as_str())
        );
    }
}
