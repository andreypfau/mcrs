use std::sync::LazyLock;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome_file::{BiomeGenerationSettings, NetworkBiome};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{
    KnownPackEntries, NetworkEntry, NetworkRegistry, NetworkTags, RegistrySet, VANILLA_PACK,
    WorldRegistries,
};
use mcrs_minecraft_world::packs::{DATAPACK_REPORT, known_pack_entries};
use mcrs_minecraft_world::registries::{static_registries, test_registries, world_registries};

use crate::common::assets;

const BIOME: &str = "minecraft:worldgen/biome";
const DIMENSION_TYPE: &str = "minecraft:dimension_type";

fn declarations() -> WorldRegistries {
    let report = std::fs::read(assets().join(DATAPACK_REPORT)).unwrap();
    world_registries(&report).unwrap_or_else(|report| panic!("{report}"))
}

fn sent(server: &RegistrySet, sends_data: impl Fn(&str, usize) -> bool) -> Vec<NetworkRegistry> {
    server
        .synced()
        .map(|(table, column)| {
            let registry = table.registry();
            NetworkRegistry {
                registry: registry.clone(),
                entries: table
                    .names()
                    .iter()
                    .zip(column)
                    .enumerate()
                    .map(|(id, (name, network))| NetworkEntry {
                        name: name.clone(),
                        data: sends_data(registry.as_str(), id).then(|| network.0.clone()),
                    })
                    .collect(),
            }
        })
        .collect()
}

fn sent_tags(statics: &RegistrySet, server: &RegistrySet) -> Vec<NetworkTags> {
    let synced: Vec<_> = server
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    server
        .tables()
        .map(|table| table.registry())
        .filter(|registry| {
            statics.table(registry.as_str()).is_some() || synced.contains(&registry.as_str())
        })
        .filter_map(|registry| server.tag_table(registry.as_str()))
        .filter(|table| !table.is_empty())
        .map(|table| NetworkTags {
            registry: table.registry().clone(),
            tags: table
                .names()
                .iter()
                .enumerate()
                .map(|(tag, name)| {
                    let members = table.members(tag).iter().map(|&m| i32::from(m)).collect();
                    (name.clone(), members)
                })
                .collect(),
        })
        .collect()
}

static KNOWN: LazyLock<KnownPackEntries> =
    LazyLock::new(|| known_pack_entries(&assets()).unwrap_or_else(|report| panic!("{report}")));

fn build(
    registries: Vec<NetworkRegistry>,
    known: Option<&KnownPackEntries>,
) -> Result<RegistrySet, String> {
    let statics = static_registries().unwrap();
    let tags = sent_tags(&statics, test_registries());
    declarations()
        .from_network(&statics, registries, &tags, known)
        .map_err(|report| report.to_string())
}

fn dimension_type(set: &RegistrySet, name: &str) -> (i32, u32) {
    let table = set.table(DIMENSION_TYPE).unwrap();
    let id = usize::from(table.number(name).unwrap());
    let dimension_type = &set.column::<DimensionType>(DIMENSION_TYPE).unwrap()[id];
    (dimension_type.min_y, dimension_type.height)
}

fn water_colour(set: &RegistrySet, name: &str) -> Option<i32> {
    let id = usize::from(set.table(BIOME).unwrap().number(name).unwrap());
    let biome = &set.column::<Biome>(BIOME).unwrap()[id];
    biome.effects.water_color.map(|colour| colour.0 as i32)
}

#[test]
fn the_network_column_rebuilds_every_synced_registry() {
    let server = test_registries();
    let registries = declarations();
    let client = build(sent(server, |_, _| true), None).unwrap_or_else(|report| panic!("{report}"));

    let mut synced = 0;
    for (table, column) in server.synced() {
        synced += 1;
        let registry = table.registry().as_str();
        let received = client
            .table(registry)
            .unwrap_or_else(|| panic!("{registry} was not rebuilt"));
        assert_eq!(received.names(), table.names(), "{registry}");

        for (name, network) in table.names().iter().zip(column) {
            let again = registries
                .reencode(server, registry, network.0.clone())
                .unwrap_or_else(|| panic!("{registry} declares no receive half"))
                .unwrap_or_else(|error| panic!("{registry}/{name}: {error}"));
            assert_eq!(again, network.0, "{registry}/{name}");
        }
    }
    assert_eq!(synced, 32);

    for name in ["minecraft:overworld", "minecraft:the_nether"] {
        assert_eq!(
            dimension_type(&client, name),
            dimension_type(server, name),
            "{name}"
        );
    }
    assert_eq!(dimension_type(&client, "minecraft:overworld"), (-64, 384));
    assert_eq!(
        water_colour(&client, "minecraft:plains"),
        water_colour(server, "minecraft:plains")
    );
    assert!(water_colour(&client, "minecraft:plains").is_some());
    assert!(client.column::<BiomeGenerationSettings>(BIOME).is_none());
}

#[test]
fn a_known_pack_entry_is_filled_from_the_local_pack() {
    let server = test_registries();
    let from_vanilla =
        |registry: &str, id: usize| server.pack_of(registry, id) == Some(VANILLA_PACK);
    let bare = sent(server, |registry, id| !from_vanilla(registry, id));
    assert!(
        bare.iter()
            .flat_map(|registry| &registry.entries)
            .any(|entry| entry.data.is_none())
    );

    let filled = build(bare, Some(&KNOWN)).unwrap_or_else(|report| panic!("{report}"));
    let full = build(sent(server, |_, _| true), None).unwrap();
    for (table, column) in server.synced() {
        let registry = table.registry().as_str();
        assert_eq!(
            filled.table(registry).unwrap().names(),
            full.table(registry).unwrap().names()
        );
        for (id, (name, network)) in table.names().iter().zip(column).enumerate() {
            if from_vanilla(registry, id) {
                assert_eq!(
                    KNOWN.get(registry, name.as_str()),
                    Some(&network.0),
                    "{registry}/{name}"
                );
            }
        }
    }
    assert!(KNOWN.get(BIOME, "minecraft:plains").is_some());
    assert_eq!(
        water_colour(&filled, "minecraft:plains"),
        water_colour(&full, "minecraft:plains")
    );
    assert_eq!(dimension_type(&filled, "minecraft:overworld"), (-64, 384));
}

#[test]
fn an_entry_without_data_outside_the_known_packs_fails() {
    let server = test_registries();
    let beta = (0..server.table(BIOME).unwrap().len())
        .find(|&id| server.pack_of(BIOME, id) == Some("beta"))
        .expect("the beta pack adds a biome");
    let beta_name = server.table(BIOME).unwrap().name(beta).unwrap().to_string();

    for known in [Some(&*KNOWN), None] {
        let report = build(
            sent(server, |registry, id| !(registry == BIOME && id == beta)),
            known,
        )
        .err()
        .expect("the entry cannot be filled");
        assert!(report.contains(&format!("{BIOME}/{beta_name}")), "{report}");
    }
}

#[test]
fn an_entry_with_data_is_decoded_even_when_known() {
    let server = test_registries();
    let from_vanilla =
        |registry: &str, id: usize| server.pack_of(registry, id) == Some(VANILLA_PACK);
    let plains = server
        .table(BIOME)
        .unwrap()
        .number("minecraft:plains")
        .unwrap();
    let original = server
        .synced()
        .find(|(table, _)| table.registry().as_str() == BIOME)
        .map(|(_, column)| column[usize::from(plains)].0.clone())
        .unwrap();
    let mut changed: NetworkBiome = mcrs_minecraft_nbt::from_tag(original.clone()).unwrap();
    changed.temperature = 0.123;
    let changed: NbtTag = mcrs_minecraft_nbt::to_nbt_tag(&changed).unwrap();
    assert_ne!(changed, original);

    let mut registries = sent(server, |registry, id| !from_vanilla(registry, id));
    for registry in &mut registries {
        for entry in &mut registry.entries {
            if registry.registry.as_str() == BIOME && entry.name.as_str() == "minecraft:plains" {
                entry.data = Some(changed.clone());
            }
        }
    }
    let set = build(registries, Some(&KNOWN)).unwrap_or_else(|report| panic!("{report}"));
    let biomes = set.column::<Biome>(BIOME).unwrap();
    assert_eq!(biomes[usize::from(plains)].temperature, 0.123);
    let desert = usize::from(
        set.table(BIOME)
            .unwrap()
            .number("minecraft:desert")
            .unwrap(),
    );
    assert_ne!(biomes[desert].temperature, 0.123);
}
