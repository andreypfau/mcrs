use bevy_app::App;
use mcrs_minecraft_assets::snapshot::RegistrySnapshot;
use mcrs_minecraft_assets::tag::DynTagRegistry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_registry::key;

fn members(app: &App, tag: &str) -> Vec<String> {
    let tags = app.world().resource::<DynTagRegistry<key::Biome>>();
    let index = app.world().resource::<DynRegistryIndex<key::Biome>>();
    let key = TagKey::<key::Biome, _>::from_location(ResourceLocation::parse(tag).unwrap());
    let mut names: Vec<String> = tags
        .get(&key)
        .expect("the tag is resolved")
        .iter()
        .map(|id| {
            index
                .location(id)
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

pub fn the_biome_index_and_snapshot_agree_on_the_id_space(app: &App) {
    let index = app.world().resource::<DynRegistryIndex<key::Biome>>();
    let snapshot = app.world().resource::<RegistrySnapshot<Biome>>();

    assert_eq!(index.len(), snapshot.len());
    assert!(!index.is_empty());
    for (id, entry) in snapshot.iter() {
        assert_eq!(
            index.location(id).map(|l| l.as_str()),
            Some(entry.location.as_str())
        );
    }
}
