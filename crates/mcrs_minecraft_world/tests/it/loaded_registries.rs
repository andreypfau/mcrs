use mcrs_minecraft_biome_file::{BiomeFile, BiomeGenerationSettings};
use mcrs_minecraft_environment::attribute::EnvironmentAttributeMap;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::memory::{Dir, MemoryAssetReader};
use bevy_asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use mcrs_minecraft_assets::RegistryAccess;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, PackLayers, VANILLA_PACK, layered_file_source};
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::{ResourceLocation, TagKey, rl};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::dialog::{Action, Dialog, DialogBody, Input};
use mcrs_minecraft_item::{BannerPattern, InstrumentValue, PaintingVariantValue};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{HolderSet, Id, Pack, PackFile, RegistrySet, TagId, WorldRegistries};
use mcrs_minecraft_sound::SoundEvent;
use mcrs_minecraft_world::enchantment_provider::EnchantmentProvider;
use mcrs_minecraft_world::registries::{
    read_packs, register_split_registries, static_registries as build_static_registries,
    test_registries, world_registries,
};
use mcrs_minecraft_world::sulfur_cube_archetype::SulfurCubeArchetype;
use mcrs_minecraft_world::test_types::{TestEnvironment, TestInstance};
use mcrs_minecraft_world::villager_trade::VillagerTrade;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_worldgen_testing::packs;
use std::sync::LazyLock;

use crate::common::{assets, datapack_report, declared_world_registries, loaded_names};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;

static STATICS: LazyLock<RegistrySet> = LazyLock::new(|| build_static_registries().unwrap());

#[test]
fn the_generated_static_set_equals_the_report() {
    let report =
        from_report(&std::fs::read(assets().join("mcrs/reports/registries.json")).unwrap())
            .unwrap();
    let built = build_static_registries().unwrap();

    assert_eq!(built.tables().count(), report.tables().count());
    for stored in report.tables() {
        let registry = stored.registry().as_str();
        let table = built
            .table(registry)
            .unwrap_or_else(|| panic!("{registry} is missing from the generated set"));
        assert_eq!(table.names(), stored.names(), "{registry}");
        for (id, name) in stored.names().iter().enumerate() {
            assert_eq!(
                table.number(name.as_str()),
                u16::try_from(id).ok(),
                "{name}"
            );
        }
    }

    let again = build_static_registries().unwrap();
    for table in built.tables() {
        let twin = again.table(table.registry().as_str()).unwrap();
        assert_eq!(twin.names(), table.names(), "{}", table.registry());
    }

    let with_empty = RegistrySet::from_locations(&[
        (rl!("minecraft:none"), &[]),
        (
            rl!("minecraft:some"),
            &[rl!("minecraft:a"), rl!("minecraft:b")],
        ),
    ])
    .unwrap();
    let none = with_empty.table("minecraft:none").unwrap();
    assert!(none.is_empty());
    assert_eq!(none.len(), 0);
    assert_eq!(with_empty.table("minecraft:some").unwrap().len(), 2);
}

static WORLD: LazyLock<WorldRegistries> = LazyLock::new(|| {
    let bytes = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    world_registries(&bytes).expect("the report parses")
});

fn load_text(files: &[(&str, &str)]) -> String {
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files: files
            .iter()
            .map(|(path, json)| PackFile {
                path: (*path).to_owned(),
                bytes: Some(json.as_bytes().to_vec()),
            })
            .collect(),
        built: Vec::new(),
    }];
    WORLD
        .load(&STATICS, &packs)
        .err()
        .map(|report| report.to_string())
        .unwrap_or_default()
}

fn refused_by_the_loader(registry: &str, name: &str, json: &str) -> String {
    load_text(&[(&format!("minecraft/{registry}/{name}.json"), json)])
}

fn instrument(sound: &str) -> String {
    format!(
        r#"{{"sound_event":"{sound}","use_duration":7.0,"range":256.0,
        "description":{{"translate":"instrument.minecraft.ponder_goat_horn"}}}}"#
    )
}

fn static_registries() -> BTreeSet<String> {
    let path = assets().join("mcrs/reports/registries.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let report: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    report.into_keys().collect()
}

#[test]
fn the_declared_registries_are_the_reports_world_registries() {
    let report = datapack_report();
    let statics = static_registries();
    let set = test_registries();

    let world: BTreeSet<String> = set
        .tables()
        .map(|table| table.registry().to_string())
        .filter(|registry| !statics.contains(registry))
        .collect();
    assert_eq!(world, declared_world_registries());

    for (registry, flags) in &report.registries {
        if flags.stable {
            assert!(!world.contains(registry), "{registry} is stable");
        }
        let directory = assets()
            .join("minecraft")
            .join(registry.trim_start_matches("minecraft:"));
        if directory.is_dir() {
            assert!(
                world.contains(registry) || flags.stable,
                "{registry} has files under assets/minecraft and is neither declared nor stable"
            );
        }
    }
}

const PARSED_WORLDGEN: [&str; 4] = [
    "minecraft:worldgen/biome",
    "minecraft:worldgen/block_state_provider",
    "minecraft:worldgen/multi_noise_biome_source_parameter_list",
    "minecraft:worldgen/world_preset",
];

#[test]
fn the_parsed_registries_are_the_reports_world_registries_outside_unparsed_worldgen() {
    let world = &*WORLD;
    let parsed: BTreeSet<String> = world
        .declared()
        .filter(|registry| world.parses(registry.as_str()))
        .map(|registry| registry.to_string())
        .collect();
    let expected: BTreeSet<String> = datapack_report()
        .registries
        .into_iter()
        .filter(|(registry, flags)| {
            flags.elements
                && !flags.stable
                && (!registry.starts_with("minecraft:worldgen/")
                    || PARSED_WORLDGEN.contains(&registry.as_str()))
                && registry.as_str() != "minecraft:trial_spawner"
        })
        .map(|(registry, _)| registry)
        .collect();

    let unparsed: Vec<_> = expected.difference(&parsed).collect();
    let unexpected: Vec<_> = parsed.difference(&expected).collect();
    assert!(
        unparsed.is_empty() && unexpected.is_empty(),
        "world registries the loader does not parse: {unparsed:?}; registries it parses that are not world registries outside worldgen: {unexpected:?}"
    );
}

#[test]
fn every_shipped_and_builtin_biome_has_a_name_from_the_loader() {
    let set = test_registries();
    let biomes = set.table("minecraft:worldgen/biome").expect("biome table");
    let names: BTreeSet<String> = biomes.names().iter().map(|name| name.to_string()).collect();
    let built = mcrs_minecraft_worldgen_builtin::biomes(set).expect("the built biomes resolve");
    assert!(!built.is_empty());
    let files: Vec<String> = std::iter::once(assets())
        .chain(packs())
        .filter_map(|root| std::fs::read_dir(root.join("minecraft/worldgen/biome")).ok())
        .flatten()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            Some(format!("minecraft:{}", name.strip_suffix(".json")?))
        })
        .collect();
    assert!(!files.is_empty());
    for expected in built.keys().map(|name| name.to_string()).chain(files) {
        assert!(
            names.contains(&expected),
            "{expected} is missing from the biome names"
        );
    }
}

#[test]
fn vanilla_biomes_are_built_entries_not_files() {
    let set = test_registries();
    let table = set.table("minecraft:worldgen/biome").expect("biome table");
    let vanilla = (0..table.len())
        .filter(|&id| set.pack_of("minecraft:worldgen/biome", id) == Some(VANILLA_PACK))
        .count();
    assert_eq!(vanilla, 67);
    assert!(
        mcrs_minecraft_worldgen_builtin::asset("minecraft/worldgen/biome/plains.json").is_none(),
        "a built biome is served as a file"
    );
    assert!(
        mcrs_minecraft_worldgen_builtin::paths("minecraft/worldgen/biome").is_empty(),
        "a built biome is listed as a file"
    );
}

#[test]
fn a_biome_naming_a_missing_carver_or_placed_feature_fails_the_load() {
    let cases = [
        (
            "carvers",
            r#""test:no_such_carver""#,
            "minecraft:worldgen/carver",
        ),
        (
            "features",
            r#"[["test:no_such_feature"]]"#,
            "minecraft:worldgen/placed_feature",
        ),
    ];
    for (field, value, registry) in cases {
        let biome = format!(
            r##"{{"temperature":0.5,"downfall":0.5,"has_precipitation":true,"effects":{{"water_color":"#3f76e4"}},"{field}":{value}}}"##
        );
        let refused = refused_by_the_loader("worldgen/biome", "test_biome", &biome);
        let line = refused
            .lines()
            .find(|line| line.contains("test_biome"))
            .unwrap_or_else(|| panic!("{field}: no line names the biome: {refused}"));
        assert!(line.contains(registry), "{field}: {line}");
        assert!(line.contains("test:no_such_"), "{field}: {line}");
    }
}

#[test]
fn a_preset_naming_a_missing_biome_fails_the_load() {
    use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;

    let missing = overworld_parameter_list().values()[0].1;
    let biome_files: Vec<(String, String)> = std::iter::once(assets())
        .chain(packs())
        .filter_map(|root| std::fs::read_dir(root.join("minecraft/worldgen/biome")).ok())
        .flatten()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|file| format!("minecraft:{}", file.trim_end_matches(".json")) != missing)
        .map(|file| (file.clone(), file))
        .collect();
    assert!(!biome_files.is_empty());

    let biome_json = |file: &str| {
        let path = std::iter::once(assets())
            .chain(packs())
            .map(|root| root.join("minecraft/worldgen/biome").join(file))
            .find(|path| path.is_file())
            .unwrap();
        std::fs::read_to_string(path).unwrap()
    };
    let mut files: Vec<(String, String)> = biome_files
        .iter()
        .map(|(file, _)| (format!("minecraft/worldgen/biome/{file}"), biome_json(file)))
        .collect();
    files.push((
        "minecraft/worldgen/multi_noise_biome_source_parameter_list/overworld.json".to_owned(),
        r#"{"preset":"minecraft:overworld"}"#.to_owned(),
    ));
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, json)| (path.as_str(), json.as_str()))
        .collect();

    let refused = load_text(&files);
    let line = refused
        .lines()
        .find(|line| line.contains("overworld") && line.contains(missing))
        .unwrap_or_else(|| panic!("no line names the parameter list and {missing}: {refused}"));
    assert!(
        line.contains("minecraft:worldgen/multi_noise_biome_source_parameter_list"),
        "{line}"
    );
}

#[test]
fn the_banner_pattern_column_follows_the_name_table() {
    let set = test_registries();
    let table = set
        .table("minecraft:banner_pattern")
        .expect("banner pattern table");
    let column = set
        .column::<BannerPattern>("minecraft:banner_pattern")
        .expect("banner patterns are parsed by the loader");
    assert_eq!(column.len(), table.len());
    assert!(!column.is_empty());
    for (name, pattern) in table.names().iter().zip(column) {
        assert_eq!(&pattern.asset_id.to_string(), &name.to_string());
    }
}

#[test]
fn a_local_entry_comes_from_its_pack() {
    let set = test_registries();
    for (registry, name, pack) in [
        ("minecraft:dimension_type", "minecraft:beta", "beta"),
        ("minecraft:dimension_type", "minecraft:overworld", "vanilla"),
        ("minecraft:worldgen/world_preset", "minecraft:beta", "beta"),
        (
            "minecraft:worldgen/world_preset",
            "minecraft:normal",
            "vanilla",
        ),
    ] {
        let id = set
            .table(registry)
            .and_then(|table| table.number(name))
            .unwrap_or_else(|| panic!("{registry} has no {name}"));
        assert_eq!(set.pack_of(registry, id as usize), Some(pack), "{name}");
    }
}

#[test]
fn packs_follow_vanilla_in_name_order() {
    let pattern =
        |name: &str| format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#);
    let root = Dir::default();
    root.insert_asset_text(Path::new("minecraft/banner_pattern/a.json"), &pattern("a"));
    root.insert_asset_text(
        Path::new("mcrs/datapacks/extra/minecraft/banner_pattern/b.json"),
        &pattern("b"),
    );

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || {
            Box::new(PackLayers::new(Box::new(MemoryAssetReader {
                root: root.clone(),
            })))
        }),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    let asset_server = app.world().resource::<AssetServer>().clone();

    let packs = read_packs(&asset_server, &WORLD, &RegistrySet::new());

    let listed: Vec<_> = packs
        .iter()
        .map(|pack| {
            (
                pack.name.as_str(),
                pack.files
                    .iter()
                    .filter(|file| file.bytes.is_some())
                    .map(|file| (file.path.as_str(), file.bytes.as_deref()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "vanilla",
                vec![(
                    "minecraft/banner_pattern/a.json",
                    Some(pattern("a").as_bytes())
                )]
            ),
            (
                "extra",
                vec![(
                    "minecraft/banner_pattern/b.json",
                    Some(pattern("b").as_bytes())
                )]
            ),
        ]
    );

    let source = asset_server.get_source(AssetSourceId::Default).unwrap();
    let bytes = bevy_tasks::block_on(read_whole(
        source.reader(),
        Path::new("minecraft/banner_pattern/b.json"),
    ))
    .expect("the layered reader reads the pack's file at its virtual path");
    assert_eq!(bytes, pattern("b").as_bytes());
}

fn shipped_file(
    pack: &str,
    registry_path: &str,
    namespace: &str,
    path: &str,
) -> std::path::PathBuf {
    let root = if pack == VANILLA_PACK {
        assets()
    } else {
        assets().join(PACKS_ROOT).join(pack)
    };
    root.join(namespace)
        .join(registry_path)
        .join(format!("{path}.json"))
}

const SHIPPED_EMPTY: [&str; 1] = ["minecraft:dimension"];

#[test]
fn every_shipped_file_of_a_parsed_registry_round_trips() {
    let set = test_registries();
    let world = &*WORLD;
    let mut parsed = 0;
    for registry in world.declared().filter(|r| world.parses(r.as_str())) {
        parsed += 1;
        let table = set
            .table(registry.as_str())
            .unwrap_or_else(|| panic!("{registry} has no table"));
        if SHIPPED_EMPTY.contains(&registry.as_str()) {
            assert!(
                table.is_empty(),
                "{registry} is shipped empty and has entries"
            );
            continue;
        }
        assert!(!table.is_empty(), "{registry} parses and has no entries");
        for (index, name) in table.names().iter().enumerate() {
            let pack = set
                .pack_of(registry.as_str(), index)
                .unwrap_or_else(|| panic!("{registry}/{name} names no pack"));
            let file = shipped_file(pack, registry.path(), name.namespace(), name.path());
            let shipped = std::fs::read_to_string(&file);
            let encoded = world
                .encode(set, registry.as_str(), index)
                .unwrap_or_else(|| panic!("{registry}/{name} has no encoding"))
                .unwrap_or_else(|e| panic!("{registry}/{name} does not encode: {e}"));
            let Ok(text) = shipped else {
                assert!(
                    pack == VANILLA_PACK && registry.as_str() == "minecraft:worldgen/biome",
                    "{registry}/{name}: {} is not a file and the entry is not a built biome",
                    file.display()
                );
                let read: BiomeFile = set.scope(|| serde_json::from_str(&encoded).unwrap());
                let stored = BiomeFile::join((
                    &set.column::<Biome>(registry.as_str()).unwrap()[index],
                    &set.column::<EnvironmentAttributeMap>(registry.as_str())
                        .unwrap()[index],
                    &set.column::<BiomeGenerationSettings>(registry.as_str())
                        .unwrap()[index],
                ));
                assert!(read == stored, "{registry}/{name} reads back changed");
                continue;
            };
            let from_file: serde_json::Value = serde_json::from_str(&text).unwrap();
            let from_entry: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(
                from_entry,
                from_file,
                "{registry}/{name} ({})",
                file.display()
            );
        }
    }
    assert!(parsed > 0, "the loader parses no registry");
}

#[test]
fn a_loaded_holder_set_answers_membership_through_the_tags() {
    let set = test_registries();
    let archetypes = set
        .column::<SulfurCubeArchetype>("minecraft:sulfur_cube_archetype")
        .expect("the archetypes are parsed by the loader");
    let items = set.registry::<keys::Item>().unwrap();
    let tags = set.tags::<keys::Item>().unwrap();
    assert!(!archetypes.is_empty());
    for archetype in archetypes {
        let first = archetype
            .items
            .ids(&tags)
            .next()
            .expect("an archetype names at least one item");
        assert!(archetype.items.contains(first, &tags));
        let outside = items
            .ids()
            .find(|id| !archetype.items.ids(&tags).any(|member| member == *id))
            .expect("an item lies outside the set");
        assert!(!archetype.items.contains(outside, &tags));
    }
}

#[test]
fn the_local_light_tag_reaches_the_loaded_item_tags() {
    let set = test_registries();
    let items = set.registry::<keys::Item>().unwrap();
    let tags = set.tags::<keys::Item>().unwrap();
    let tag = tags
        .get(&TagKey::<keys::Item, _>::from_location(
            ResourceLocation::read("mcrs:water_sensitive_light").unwrap(),
        ))
        .expect("the local light tag is loaded");
    let members: Vec<&str> = tags
        .members(tag)
        .map(|id| items.name(id).unwrap().as_str())
        .collect();
    assert_eq!(
        members,
        [
            "minecraft:torch",
            "minecraft:soul_torch",
            "minecraft:copper_torch",
            "minecraft:redstone_torch",
            "minecraft:campfire",
            "minecraft:soul_campfire",
            "minecraft:lava_bucket",
            "minecraft:fire_charge",
        ]
    );
}

#[test]
fn a_bare_name_takes_the_default_namespace() {
    test_registries().scope(|| {
        let bare: InstrumentValue =
            serde_json::from_str(&instrument("item.goat_horn.sound.0")).unwrap();
        let written = serde_json::to_value(&bare).unwrap();
        assert_eq!(written["sound_event"], "minecraft:item.goat_horn.sound.0");

        let upper = serde_json::from_str::<InstrumentValue>(&instrument(
            "minecraft:Item.goat_horn.sound.0",
        ));
        assert!(upper.is_err());
    });
}

#[test]
fn a_parse_without_a_scope_fails_on_another_thread() {
    let name = "\"minecraft:item.goat_horn.sound.0\"";
    test_registries().scope(|| {
        assert!(serde_json::from_str::<Id<SoundEvent>>(name).is_ok());
        let other = std::thread::scope(|threads| {
            threads
                .spawn(|| serde_json::from_str::<Id<SoundEvent>>(name).map_err(|e| e.to_string()))
                .join()
                .unwrap()
        });
        let message = other.unwrap_err();
        assert!(message.contains("no registry scope"), "{message}");
        assert!(message.contains("SoundEvent"), "{message}");
    });
}

#[test]
fn an_instrument_or_painting_the_game_refuses_fails_to_parse() {
    test_registries().scope(an_instrument_or_painting_the_game_refuses_fails_to_parse_in_scope);
}

fn an_instrument_or_painting_the_game_refuses_fails_to_parse_in_scope() {
    let instrument = |extra: &str| {
        format!(
            r#"{{
                "sound_event": "minecraft:item.goat_horn.sound.0",
                "use_duration": 7.0,
                "range": 256.0,
                "description": {{"translate": "instrument.minecraft.ponder_goat_horn"}}
                {extra}
            }}"#
        )
    };
    assert!(serde_json::from_str::<InstrumentValue>(&instrument("")).is_ok());
    assert!(serde_json::from_str::<InstrumentValue>(&instrument(r#", "volume": 1.0"#)).is_err());

    let painting =
        |width: u32| format!(r#"{{"asset_id": "minecraft:kebab", "width": {width}, "height": 1}}"#);
    assert!(serde_json::from_str::<PaintingVariantValue>(&painting(16)).is_ok());
    assert!(serde_json::from_str::<PaintingVariantValue>(&painting(17)).is_err());
}

#[test]
fn a_file_the_game_refuses_names_its_registry_entry_and_file() {
    let stone = r#"{"id":"minecraft:stone"}"#;
    let wolf_assets = r#"{"wild":"minecraft:a","tame":"minecraft:b","angry":"minecraft:c"}"#;
    let cases: [(&str, String, &[&str]); 18] = [
        ("instrument", "{}".to_owned(), &["sound_event"]),
        (
            "instrument",
            instrument("minecraft:stick"),
            &["minecraft:sound_event", "minecraft:stick"],
        ),
        (
            "damage_type",
            r#"{"message_id":"x","scaling":"never","exhaustion":0.0,"bogus":1}"#.to_owned(),
            &["bogus"],
        ),
        (
            "damage_type",
            r#"{"message_id":"x","scaling":"sometimes","exhaustion":0.0}"#.to_owned(),
            &["sometimes", "when_caused_by_living_non_player"],
        ),
        (
            "decorated_pot_pattern",
            r#"{"asset_id":"minecraft:x_pottery_pattern","bogus":1}"#.to_owned(),
            &["bogus"],
        ),
        (
            "block_transformer",
            format!(r#"[{{"block_state_provider":{stone},"bogus":1}}]"#),
            &["bogus"],
        ),
        (
            "chat_type",
            r#"{"bogus":1,"chat":{"translation_key":"k","parameters":[]},
                "narration":{"translation_key":"k","parameters":[]}}"#
                .to_owned(),
            &["bogus"],
        ),
        (
            "test_environment",
            r#"{"bogus":1,"type":"minecraft:weather","weather":"clear"}"#.to_owned(),
            &["bogus"],
        ),
        (
            "test_instance",
            r#"{"bogus":1,"type":"minecraft:block_based","environment":"minecraft:default",
                "structure":"minecraft:empty","max_ticks":1}"#
                .to_owned(),
            &["bogus"],
        ),
        (
            "wolf_variant",
            format!(
                r##"{{"assets":{wolf_assets},"baby_assets":{wolf_assets},"spawn_conditions":[
                    {{"condition":{{"type":"minecraft:biome","biomes":"#minecraft:no_such_tag"}},"priority":1}}]}}"##
            ),
            &["minecraft:worldgen/biome", "minecraft:no_such_tag"],
        ),
        (
            "enchantment",
            enchantment_json(r##""#minecraft:nowhere""##, ""),
            &["minecraft:item", "minecraft:nowhere"],
        ),
        (
            "sulfur_cube_archetype",
            archetype_json("minecraft:no_such_attribute"),
            &["minecraft:attribute", "minecraft:no_such_attribute"],
        ),
        (
            "enchantment_provider",
            r#"{"type":"minecraft:single","enchantment":"minecraft:no_such_enchantment","level":1}"#
                .to_owned(),
            &["minecraft:enchantment", "minecraft:no_such_enchantment"],
        ),
        (
            "enchantment_provider",
            r#"{"type":"minecraft:by_cost","enchantments":["minecraft:no_such_enchantment"],"cost":1}"#
                .to_owned(),
            &["minecraft:enchantment", "minecraft:no_such_enchantment"],
        ),
        (
            "enchantment_provider",
            r##"{"type":"minecraft:by_cost_with_difficulty","enchantments":"#minecraft:no_such_tag",
                "min_cost":1,"max_cost_span":1}"##
                .to_owned(),
            &["minecraft:enchantment", "minecraft:no_such_tag"],
        ),
        (
            "trade_set",
            r##"{"amount":2,"trades":"#minecraft:nowhere"}"##.to_owned(),
            &["minecraft:villager_trade", "minecraft:nowhere"],
        ),
        (
            "villager_trade",
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:no_such_item"}}"#
                .to_owned(),
            &["minecraft:item", "minecraft:no_such_item"],
        ),
        (
            "villager_trade",
            trade_with_modifier(r#"{"type":"minecraft:set_name"}"#),
            &["minecraft:set_name"],
        ),
    ];
    for (registry, json, parts) in cases {
        let text = refused_by_the_loader(registry, "odd", &json);
        let entry = [
            format!("minecraft:{registry}"),
            "minecraft:odd".to_owned(),
            format!("minecraft/{registry}/odd.json"),
        ];
        for part in entry
            .iter()
            .map(String::as_str)
            .chain(parts.iter().copied())
        {
            assert!(text.contains(part), "{part} missing for {json}:\n{text}");
        }
    }
}

#[test]
fn an_empty_biome_file_names_its_first_missing_field() {
    let text = refused_by_the_loader("worldgen/biome", "empty", "{}");
    for part in [
        "minecraft:worldgen/biome",
        "minecraft:empty",
        "minecraft/worldgen/biome/empty.json",
        "missing field `temperature`",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
    assert!(!text.contains("downfall"), "{text}");
}

const VARIANT_REGISTRIES: [&str; 13] = [
    "minecraft:wolf_variant",
    "minecraft:wolf_sound_variant",
    "minecraft:pig_variant",
    "minecraft:pig_sound_variant",
    "minecraft:cow_variant",
    "minecraft:cow_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:chicken_sound_variant",
    "minecraft:cat_variant",
    "minecraft:cat_sound_variant",
    "minecraft:frog_variant",
    "minecraft:zombie_nautilus_variant",
    "minecraft:painting_variant",
];

#[test]
fn an_empty_variant_registry_fails_the_load() {
    let text = load_text(&[]);
    for registry in VARIANT_REGISTRIES {
        let message = format!("Registry must be non-empty: {registry}");
        assert!(text.contains(&message), "{message} missing from:\n{text}");
    }
}

#[test]
fn the_synced_wolf_variant_has_no_spawn_conditions() {
    let set = test_registries();
    let mut access = RegistryAccess::default();
    register_split_registries(&mut access, set);
    let synced = access
        .iter()
        .find(|snapshot| snapshot.registry_key() == "minecraft:wolf_variant")
        .expect("the wolf variants are registered")
        .iter_entries()
        .find(|entry| entry.location.as_str() == "minecraft:pale")
        .and_then(|entry| entry.data.clone())
        .expect("the pale wolf is synced");
    let synced = synced.extract_compound().expect("a variant is a compound");
    assert!(synced.get("assets").is_some());
    assert!(synced.get("spawn_conditions").is_none());

    let index = set
        .table("minecraft:wolf_variant")
        .and_then(|table| table.number("minecraft:pale"))
        .expect("the pale wolf is loaded") as usize;
    let file: serde_json::Value = serde_json::from_str(
        &WORLD
            .encode(set, "minecraft:wolf_variant", index)
            .expect("the pale wolf has an encoding")
            .expect("the pale wolf encodes"),
    )
    .unwrap();
    assert_eq!(
        file["spawn_conditions"],
        serde_json::json!([{"priority": 0}])
    );
}

#[test]
fn a_chat_parameter_outside_the_game_set_fails() {
    let decoration = |parameter: &str| {
        format!(r#"{{"translation_key":"chat.type.text","parameters":["sender","{parameter}"]}}"#)
    };
    let chat_type = |parameter: &str| {
        format!(
            r#"{{"chat":{},"narration":{}}}"#,
            decoration(parameter),
            decoration("content")
        )
    };

    let text = refused_by_the_loader("chat_type", "odd", &chat_type("victim"));
    for part in [
        "minecraft:chat_type",
        "minecraft:odd",
        "minecraft/chat_type/odd.json",
        "victim",
        "sender",
        "target",
        "content",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text = refused_by_the_loader("chat_type", "fine", &chat_type("target"));
    assert!(!text.contains("minecraft:chat_type"), "{text}");
}

fn registered_names(registry: &str) -> BTreeSet<String> {
    loaded_names(test_registries(), registry)
        .into_iter()
        .collect()
}

fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(json: &str) -> serde_json::Value {
    test_registries().scope(|| {
        let value: T = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let text = serde_json::to_string(&value).unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

fn assert_samples_round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(
    type_registry: &str,
    samples: &[(&str, &str)],
) {
    let sampled: BTreeSet<String> = samples.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert_eq!(
        sampled,
        registered_names(type_registry),
        "a sample for each type {type_registry} registers, and no other"
    );
    for (name, json) in samples {
        let written = round_trip::<T>(json);
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(written, read, "{name}");
        assert_eq!(written["type"], *name, "{json}");
    }
}

#[test]
fn every_test_environment_type_round_trips() {
    assert_samples_round_trip::<TestEnvironment>(
        "minecraft:test_environment_definition_type",
        &[
            (
                "minecraft:all_of",
                r#"{"type":"minecraft:all_of","definitions":["minecraft:default",
                    {"type":"minecraft:weather","weather":"rain"}]}"#,
            ),
            (
                "minecraft:clock_time",
                r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":6000}"#,
            ),
            (
                "minecraft:difficulty",
                r#"{"type":"minecraft:difficulty","difficulty":"hard"}"#,
            ),
            (
                "minecraft:function",
                r#"{"type":"minecraft:function","setup":"minecraft:prepare","teardown":"minecraft:clean_up"}"#,
            ),
            (
                "minecraft:game_rules",
                r#"{"type":"minecraft:game_rules","rules":{"minecraft:pvp":false,
                    "minecraft:random_tick_speed":0,"minecraft:max_minecart_speed":1000}}"#,
            ),
            (
                "minecraft:timeline_attributes",
                r#"{"type":"minecraft:timeline_attributes","timelines":["minecraft:day"]}"#,
            ),
            (
                "minecraft:weather",
                r#"{"type":"minecraft:weather","weather":"thunder"}"#,
            ),
        ],
    );

    for json in [
        r#"{"type":"minecraft:function"}"#,
        r#"{"type":"minecraft:clock_time","clock":{},"time":0}"#,
        r#"{"type":"minecraft:all_of","definitions":[]}"#,
        r#"{"type":"minecraft:timeline_attributes","timelines":[{"clock":"minecraft:overworld"}]}"#,
    ] {
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(round_trip::<TestEnvironment>(json), read);
    }

    assert_eq!(
        round_trip::<TestEnvironment>(
            r#"{"type":"minecraft:timeline_attributes","timelines":[{"clock":"minecraft:overworld","tracks":{},"time_markers":{}}]}"#
        ),
        serde_json::json!({
            "type": "minecraft:timeline_attributes",
            "timelines": [{"clock": "minecraft:overworld"}],
        })
    );
}

#[test]
fn every_test_instance_type_round_trips() {
    assert_samples_round_trip::<TestInstance>(
        "minecraft:test_instance_type",
        &[
            (
                "minecraft:block_based",
                r#"{"type":"minecraft:block_based","environment":"minecraft:default",
                    "structure":"minecraft:empty","max_ticks":100}"#,
            ),
            (
                "minecraft:function",
                r#"{"type":"minecraft:function","function":"minecraft:always_pass",
                    "environment":{"type":"minecraft:weather","weather":"clear"},
                    "dimension":"minecraft:the_nether","structure":"minecraft:empty",
                    "max_ticks":20,"setup_ticks":2,"required":false,"rotation":"180",
                    "manual_only":true,"max_attempts":3,"required_successes":2,
                    "sky_access":true,"padding":4}"#,
            ),
        ],
    );
}

fn instance_naming(field: &str, name: &str) -> String {
    let (environment, function) = match field {
        "environment" => (name, "minecraft:always_pass"),
        _ => ("minecraft:default", name),
    };
    format!(
        r#"{{"type":"minecraft:function","function":"{function}","environment":"{environment}",
            "structure":"minecraft:empty","max_ticks":1}}"#
    )
}

fn refused_with_environment_default(instance: &str) -> String {
    load_text(&[
        (
            "minecraft/test_environment/default.json",
            r#"{"type":"minecraft:all_of","definitions":[]}"#,
        ),
        ("minecraft/test_instance/odd.json", instance),
    ])
}

#[test]
fn a_test_instance_naming_an_unknown_environment_fails() {
    let text =
        refused_with_environment_default(&instance_naming("environment", "minecraft:nowhere"));
    for part in [
        "minecraft:test_environment",
        "minecraft:nowhere",
        "minecraft:odd",
        "minecraft/test_instance/odd.json",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text =
        refused_with_environment_default(&instance_naming("environment", "minecraft:default"));
    assert!(!text.contains("minecraft:test_instance"), "{text}");
}

#[test]
fn a_test_instance_naming_a_function_outside_the_report_loads() {
    let text = refused_with_environment_default(&instance_naming("function", "minecraft:nowhere"));
    for part in ["minecraft:test_instance", "minecraft:nowhere"] {
        assert!(!text.contains(part), "{part} in:\n{text}");
    }
}

#[test]
fn test_values_the_game_refuses_fail_to_parse() {
    let refused = |registry: &str, json: &str, part: &str| {
        let text = refused_by_the_loader(registry, "odd", json);
        assert!(text.contains(part), "{part} missing from:\n{text}");
    };
    let environment =
        |body: &str| format!(r#"{{"type":"minecraft:game_rules","rules":{{{body}}}}}"#);
    refused(
        "test_environment",
        &environment(r#""minecraft:pvp":1"#),
        "minecraft:pvp",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:random_tick_speed":true"#),
        "minecraft:random_tick_speed",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:max_minecart_speed":1001"#),
        "[1:1000]",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:no_such_rule":true"#),
        "minecraft:no_such_rule",
    );
    let text = refused_by_the_loader(
        "test_environment",
        "odd",
        r#"{"type":"minecraft:clock_time","clock":"minecraft:nowhere","time":0}"#,
    );
    for part in ["minecraft:world_clock", "minecraft:nowhere"] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
    let text = load_text(&[
        ("minecraft/world_clock/overworld.json", "{}"),
        (
            "minecraft/test_environment/odd.json",
            r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":-1}"#,
        ),
    ]);
    assert!(
        text.contains("non-negative"),
        "non-negative missing from:\n{text}"
    );
    refused(
        "test_environment",
        r#"{"type":"minecraft:lightning"}"#,
        "minecraft:lightning",
    );

    let instance = |fields: &str| {
        format!(
            r#"{{"type":"minecraft:block_based","environment":"minecraft:default",
                "structure":"minecraft:empty",{fields}}}"#
        )
    };
    for (fields, part) in [
        (r#""max_ticks":0"#, "positive"),
        (r#""max_ticks":1,"padding":129"#, "[0;128]"),
        (r#""max_ticks":1,"rotation":"sideways""#, "sideways"),
    ] {
        let text = refused_with_environment_default(&instance(fields));
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn every_integer_game_rule_is_a_registered_rule() {
    let rules = registered_names("minecraft:game_rule");
    for (name, ..) in mcrs_minecraft_world::test_types::INTEGER_GAME_RULES {
        assert!(rules.contains(name), "{name} is not a game rule");
    }
}

fn synced<T: serde::de::DeserializeOwned + serde::Serialize>(
    json: &str,
) -> mcrs_minecraft_nbt::compound::NbtCompound {
    test_registries().scope(|| {
        let value: T = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        mcrs_minecraft_nbt::to_nbt_compound(&value).unwrap()
    })
}

#[test]
fn test_values_are_synced_with_the_tags_the_game_writes() {
    let instance = synced::<TestInstance>(
        r#"{"type":"minecraft:function","function":"minecraft:always_pass",
            "environment":"minecraft:default","dimension":"minecraft:the_nether",
            "structure":"minecraft:empty","max_ticks":20,"setup_ticks":2,"required":false,
            "rotation":"180","manual_only":true,"max_attempts":3,"required_successes":2,
            "sky_access":true,"padding":4}"#,
    );
    for (field, tag) in [
        ("max_ticks", NbtTag::Int(20)),
        ("setup_ticks", NbtTag::Int(2)),
        ("max_attempts", NbtTag::Int(3)),
        ("required_successes", NbtTag::Int(2)),
        ("padding", NbtTag::Int(4)),
        ("required", NbtTag::Byte(0)),
        ("manual_only", NbtTag::Byte(1)),
        ("sky_access", NbtTag::Byte(1)),
        ("rotation", NbtTag::String("180".to_owned())),
        (
            "environment",
            NbtTag::String("minecraft:default".to_owned()),
        ),
        (
            "function",
            NbtTag::String("minecraft:always_pass".to_owned()),
        ),
    ] {
        assert_eq!(instance.get(field), Some(&tag), "{field}");
    }

    let rules = synced::<TestEnvironment>(
        r#"{"type":"minecraft:game_rules","rules":{"minecraft:pvp":false,
            "minecraft:random_tick_speed":0}}"#,
    );
    let rules = rules
        .get_compound("rules")
        .expect("the rules are a compound");
    assert_eq!(rules.get("minecraft:pvp"), Some(&NbtTag::Byte(0)));
    assert_eq!(
        rules.get("minecraft:random_tick_speed"),
        Some(&NbtTag::Int(0))
    );

    let clock = synced::<TestEnvironment>(
        r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":6000}"#,
    );
    assert_eq!(clock.get("time"), Some(&NbtTag::Int(6000)));
}

fn dialog_list_in(tag: &str, dialogs: &str) -> String {
    let dialog = format!(r#"{{"type":"minecraft:dialog_list","title":"t","dialogs":"{dialogs}"}}"#);
    load_text(&[
        ("minecraft/dialog/odd.json", &dialog),
        (
            &format!("minecraft/tags/dialog/{tag}.json"),
            r#"{"values":[]}"#,
        ),
    ])
}

#[test]
fn a_dialog_list_naming_an_unknown_tag_fails() {
    let text = dialog_list_in("known", "#minecraft:nowhere");
    for part in [
        "minecraft:dialog",
        "minecraft:nowhere",
        "minecraft:odd",
        "minecraft/dialog/odd.json",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text = dialog_list_in("known", "#minecraft:known");
    assert!(!text.contains("minecraft:dialog"), "{text}");
}

const DIALOG_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:notice",
        r#"{"type":"minecraft:notice","title":{"translate":"t"},"external_title":"outer",
            "can_close_with_escape":false,"pause":false,"after_action":"none",
            "body":[{"type":"minecraft:plain_message","contents":"a"},
                {"type":"minecraft:plain_message","contents":"b","width":300}],
            "inputs":[{"type":"minecraft:text","key":"name","label":"Name"}],
            "action":{"label":"Go","tooltip":"tip","width":100,
                "action":{"type":"minecraft:run_command","command":"say hi"}}}"#,
    ),
    (
        "minecraft:confirmation",
        r#"{"type":"minecraft:confirmation","title":"Sure?",
            "body":{"type":"minecraft:plain_message","contents":"a"},
            "yes":{"label":"Yes"},
            "no":{"label":"No","action":{"type":"minecraft:show_dialog",
                "dialog":"minecraft:server_links"}}}"#,
    ),
    (
        "minecraft:multi_action",
        r#"{"type":"minecraft:multi_action","title":"Pick",
            "actions":[{"label":"A"},
                {"label":"B","action":{"type":"minecraft:change_page","page":2}}],
            "exit_action":{"label":"Back","width":200},"columns":3}"#,
    ),
    (
        "minecraft:server_links",
        r#"{"type":"minecraft:server_links","title":"Links","button_width":310,"columns":1,
            "exit_action":{"label":"Back"}}"#,
    ),
    (
        "minecraft:dialog_list",
        r#"{"type":"minecraft:dialog_list","title":"More",
            "dialogs":["minecraft:server_links","minecraft:custom_options"],
            "columns":1,"button_width":310}"#,
    ),
];

const BODY_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:item",
        r#"{"type":"minecraft:item","item":{"id":"minecraft:apple","count":2},
            "description":{"contents":"d","width":100},"show_decorations":false,
            "show_tooltip":false,"width":32,"height":64}"#,
    ),
    (
        "minecraft:plain_message",
        r#"{"type":"minecraft:plain_message","contents":{"translate":"x"},"width":300}"#,
    ),
];

const INPUT_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:boolean",
        r#"{"type":"minecraft:boolean","key":"flag","label":"F","initial":true,
            "on_true":"yes","on_false":"no"}"#,
    ),
    (
        "minecraft:number_range",
        r#"{"type":"minecraft:number_range","key":"n","width":300,"label":"N",
            "label_format":"x.y","start":0.0,"end":10.0,"initial":5.0,"step":0.5}"#,
    ),
    (
        "minecraft:single_option",
        r#"{"type":"minecraft:single_option","key":"o","width":100,
            "options":[{"id":"a","display":"A","initial":true},{"id":"b"}],
            "label":"O","label_visible":false}"#,
    ),
    (
        "minecraft:text",
        r#"{"type":"minecraft:text","key":"t","width":100,"label":"T","label_visible":false,
            "initial":"ab","max_length":10,"multiline":{"max_lines":4,"height":64}}"#,
    ),
];

const ACTION_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:open_url",
        r#"{"type":"minecraft:open_url","url":"https://example.com"}"#,
    ),
    (
        "minecraft:run_command",
        r#"{"type":"minecraft:run_command","command":"say hi"}"#,
    ),
    (
        "minecraft:suggest_command",
        r#"{"type":"minecraft:suggest_command","command":"say hi"}"#,
    ),
    (
        "minecraft:show_dialog",
        r#"{"type":"minecraft:show_dialog","dialog":"minecraft:server_links"}"#,
    ),
    (
        "minecraft:change_page",
        r#"{"type":"minecraft:change_page","page":2}"#,
    ),
    (
        "minecraft:copy_to_clipboard",
        r#"{"type":"minecraft:copy_to_clipboard","value":"v"}"#,
    ),
    (
        "minecraft:custom",
        r#"{"type":"minecraft:custom","id":"minecraft:ping","payload":{"a":1}}"#,
    ),
    (
        "minecraft:dynamic/run_command",
        r#"{"type":"minecraft:dynamic/run_command","template":"say $(who)"}"#,
    ),
    (
        "minecraft:dynamic/custom",
        r#"{"type":"minecraft:dynamic/custom","id":"minecraft:ping","additions":{"a":"b"}}"#,
    ),
];

#[test]
fn every_dialog_type_round_trips() {
    assert_samples_round_trip::<Dialog>("minecraft:dialog_type", DIALOG_SAMPLES);
    assert_samples_round_trip::<DialogBody>("minecraft:dialog_body_type", BODY_SAMPLES);
    assert_samples_round_trip::<Input>("minecraft:input_control_type", INPUT_SAMPLES);
    assert_samples_round_trip::<Action>("minecraft:dialog_action_type", ACTION_SAMPLES);

    let read = |json: &str| round_trip::<Dialog>(json);
    assert_eq!(
        read(
            r#"{"type":"minecraft:notice","title":"t",
                "body":{"type":"minecraft:item","item":"minecraft:apple","description":"d"},
                "inputs":[{"type":"minecraft:single_option","key":"o","label":"O",
                    "options":["a","b"]}],
                "action":{"label":"ok","action":{"type":"run_command","command":"x"}}}"#
        ),
        serde_json::json!({
            "type": "minecraft:notice",
            "title": "t",
            "body": {"type": "minecraft:item", "item": {"id": "minecraft:apple"},
                "description": {"contents": "d"}},
            "inputs": [{"type": "minecraft:single_option", "key": "o", "label": "O",
                "options": [{"id": "a"}, {"id": "b"}]}],
            "action": {"label": "ok",
                "action": {"type": "minecraft:run_command", "command": "x"}},
        }),
        "alternative spellings are read as the game reads them and written in its primary form"
    );
}

#[test]
fn a_dialog_value_outside_its_range_fails() {
    let dialog = |fields: &str| format!(r#"{{"title":"t",{fields}}}"#);
    let notice = |fields: &str| dialog(&format!(r#""type":"minecraft:notice",{fields}"#));
    let input = |control: &str| {
        notice(&format!(
            r#""inputs":[{{"key":"k","label":"L",{control}}}]"#
        ))
    };
    let action = |action: &str| notice(&format!(r#""action":{{"label":"x","action":{action}}}"#));

    for (json, part) in [
        (notice(r#""action":{"label":"x","width":0}"#), "[1;1024]"),
        (notice(r#""action":{"label":"x","width":1025}"#), "[1;1024]"),
        (
            dialog(r#""type":"minecraft:server_links","columns":0"#),
            "positive",
        ),
        (
            dialog(r#""type":"minecraft:server_links","button_width":0"#),
            "[1;1024]",
        ),
        (
            input(r#""type":"minecraft:number_range","start":0.0,"end":1.0,"step":0.0"#),
            "positive",
        ),
        (
            input(r#""type":"minecraft:number_range","start":0.0,"end":1.0,"initial":2.0"#),
            "outside of range",
        ),
        (
            input(r#""type":"minecraft:text","max_length":0"#),
            "positive",
        ),
        (
            input(r#""type":"minecraft:text","initial":"abc","max_length":2"#),
            "exceeds allowed size",
        ),
        (
            input(r#""type":"minecraft:single_option","options":[]"#),
            "must have contents",
        ),
        (
            input(
                r#""type":"minecraft:single_option","options":[{"id":"a","initial":true},{"id":"b","initial":true}]"#,
            ),
            "Multiple initial values",
        ),
        (input(r#""type":"minecraft:boolean","bogus":1"#), "bogus"),
        (
            notice(r#""inputs":[{"type":"minecraft:boolean","key":"a b","label":"L"}]"#),
            "not a valid input name",
        ),
        (
            dialog(r#""type":"minecraft:multi_action","actions":[]"#),
            "must have contents",
        ),
        (notice(r#""pause":true,"after_action":"none""#), "unpause"),
        (notice(r#""bogus":1"#), "bogus"),
        (notice(r#""after_action":"later""#), "later"),
        (
            dialog(r#""type":"minecraft:lightning""#),
            "minecraft:lightning",
        ),
        (
            notice(r#""body":{"type":"minecraft:plain_message","contents":"a","width":0}"#),
            "[1;1024]",
        ),
        (
            notice(r#""body":{"type":"minecraft:item","item":"minecraft:apple","width":257}"#),
            "[1;256]",
        ),
        (
            notice(
                r#""body":{"type":"minecraft:item","item":"minecraft:apple","description":{"contents":"d","width":0}}"#,
            ),
            "did not match any variant",
        ),
        (
            action(r#"{"type":"minecraft:run_command","command":"x","bogus":1}"#),
            "bogus",
        ),
        (
            action(r#"{"type":"minecraft:open_file","path":"x"}"#),
            "open_file",
        ),
        (
            action(r#"{"type":"minecraft:change_page","page":0}"#),
            "positive",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/run_command","template":"no variables"}"#),
            "No variables in macro",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/run_command","template":"say $(who"}"#),
            "Unterminated macro variable",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/custom","id":"minecraft:x","bogus":1}"#),
            "bogus",
        ),
    ] {
        let text = refused_by_the_loader("dialog", "odd", &json);
        for part in [part, "minecraft:odd"] {
            assert!(text.contains(part), "{part} missing for {json}:\n{text}");
        }
    }
}

#[test]
fn dialog_values_are_synced_with_the_tags_the_game_writes() {
    let dialog = synced::<Dialog>(
        r#"{"type":"minecraft:multi_action","title":"Pick",
            "actions":[{"label":"A","width":100,
                "action":{"type":"minecraft:change_page","page":2}}],
            "inputs":[{"type":"minecraft:number_range","key":"n","label":"N",
                "start":0.0,"end":10.0,"step":0.5}],
            "pause":false,"after_action":"none","columns":3}"#,
    );
    for (field, tag) in [
        ("columns", NbtTag::Int(3)),
        ("pause", NbtTag::Byte(0)),
        ("after_action", NbtTag::String("none".to_owned())),
    ] {
        assert_eq!(dialog.get(field), Some(&tag), "{field}");
    }
    let NbtTag::List(actions) = dialog.get("actions").expect("the actions are a list") else {
        panic!("the actions are not a list");
    };
    let NbtTag::Compound(button) = &actions[0] else {
        panic!("a button is not a compound");
    };
    assert_eq!(button.get("width"), Some(&NbtTag::Int(100)));
    let NbtTag::Compound(click) = button.get("action").expect("the button acts") else {
        panic!("an action is not a compound");
    };
    assert_eq!(click.get("page"), Some(&NbtTag::Int(2)));
    assert_eq!(
        click.get("type"),
        Some(&NbtTag::String("minecraft:change_page".to_owned()))
    );
    let NbtTag::List(inputs) = dialog.get("inputs").expect("the inputs are a list") else {
        panic!("the inputs are not a list");
    };
    let NbtTag::Compound(range) = &inputs[0] else {
        panic!("an input is not a compound");
    };
    assert_eq!(range.get("start"), Some(&NbtTag::Float(0.0)));
    assert_eq!(range.get("step"), Some(&NbtTag::Float(0.5)));
}

fn enchantment_json(supported_items: &str, effects: &str) -> String {
    format!(
        r#"{{"description":{{"translate":"enchantment.minecraft.odd"}},
        "min_cost":{{"base":1,"per_level_above_first":1}},
        "max_cost":{{"base":2,"per_level_above_first":1}},
        "anvil_cost":1,"slots":["mainhand"],"supported_items":{supported_items},
        "weight":1,"max_level":1{effects}}}"#
    )
}

fn replacing_with(block_state: &str) -> String {
    format!(
        r#","effects":{{"minecraft:location_changed":[{{"effect":{{
        "type":"minecraft:replace_disk","radius":1.0,"height":1.0,
        "block_state":{block_state}}}}}]}}"#
    )
}

#[test]
fn a_block_state_provider_reference_is_checked() {
    let text = refused_by_the_loader(
        "enchantment",
        "odd",
        &enchantment_json(
            r#""minecraft:stick""#,
            &replacing_with(r#""minecraft:no_such_provider""#),
        ),
    );
    for part in [
        "minecraft:worldgen/block_state_provider",
        "minecraft:no_such_provider",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    use mcrs_minecraft_enchantment::effects::{BlockState, BlockStateProvider};
    let set = test_registries();
    let provider = set
        .table("minecraft:worldgen/block_state_provider")
        .and_then(|table| table.names().first())
        .expect("the data pack ships a block state provider")
        .to_string();
    set.scope(|| {
        let provider = format!("\"{provider}\"");
        for json in [
            provider.as_str(),
            r#"{"id":"minecraft:frosted_ice","properties":{"age":"0"}}"#,
            r#"{"type":"minecraft:simple","state":"minecraft:stone"}"#,
            r#"{"type":"minecraft:simple","state":{"id":"minecraft:stone"}}"#,
        ] {
            let read: BlockStateProvider =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&serde_json::to_string(&read).unwrap())
                    .unwrap(),
                serde_json::from_str::<serde_json::Value>(json).unwrap(),
                "{json}"
            );
        }
        for json in [
            r#""minecraft:stone""#,
            r#"{"id":"minecraft:stone","properties":{}}"#,
        ] {
            let read: BlockState =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(serde_json::to_string(&read).unwrap(), json, "{json}");
        }
        for json in [
            r#""minecraft:no_such_block""#,
            r#"{"id":"minecraft:no_such_block"}"#,
        ] {
            let message = serde_json::from_str::<BlockState>(json)
                .expect_err(json)
                .to_string();
            assert!(message.contains("minecraft:block"), "{message}");
            assert!(message.contains("minecraft:no_such_block"), "{message}");
        }
        for json in [
            r#"{"type":"minecraft:simple","id":"minecraft:stone"}"#,
            r#"{"type":"minecraft:nowhere","state":"minecraft:stone"}"#,
            r#"{"id":"minecraft:stone","state":"minecraft:stone"}"#,
            r#"{"id":"minecraft:stone","bogus":1}"#,
        ] {
            serde_json::from_str::<BlockStateProvider>(json).expect_err(json);
        }
    });
}

fn archetype_json(attribute: &str) -> String {
    format!(
        r#"{{"attribute_modifiers":[{{"amount":-1.0,"attribute":"{attribute}",
        "id":"minecraft:odd_add_knockback_resistance","operation":"add_value"}}],
        "items":"minecraft:slime_ball",
        "knockback_modifiers":{{"horizontal_power":0.4,"vertical_power":0.1}},
        "sound_settings":{{"hit_sound":"minecraft:entity.sulfur_cube.regular.hit",
        "push_sound":"minecraft:entity.sulfur_cube.regular.push",
        "push_sound_cooldown":0.5,"push_sound_impulse_threshold":0.2}}}}"#
    )
}

#[test]
fn an_archetype_the_game_refuses_fails_to_parse() {
    let valid: serde_json::Value =
        serde_json::from_str(&archetype_json("minecraft:knockback_resistance")).unwrap();
    assert_eq!(round_trip::<SulfurCubeArchetype>(&valid.to_string()), valid);

    let mut full = valid.clone();
    full["buoyant"] = serde_json::json!(true);
    full["explosion"] = serde_json::json!({"power": 3, "causes_fire": false, "fuse": 120});
    full["contact_damage"] = serde_json::json!({
        "damage_type": "minecraft:sulfur_cube_hot",
        "amount": {"type": "minecraft:uniform", "min_inclusive": 0.5, "max_exclusive": 1.5},
        "attribute_to_source": true,
    });
    assert_eq!(round_trip::<SulfurCubeArchetype>(&full.to_string()), full);

    let refused = [
        (
            "/explosion",
            serde_json::json!({"power": -1, "causes_fire": false, "fuse": 10}),
            "non-negative",
        ),
        (
            "/explosion",
            serde_json::json!({"power": 1, "causes_fire": false, "fuse": 0}),
            "positive",
        ),
        (
            "/contact_damage",
            serde_json::json!({"damage_type": "minecraft:sulfur_cube_hot", "amount": -1.0,
                "attribute_to_source": false}),
            "too low",
        ),
        (
            "/contact_damage",
            serde_json::json!({"damage_type": "minecraft:no_such_damage", "amount": 1.0,
                "attribute_to_source": false}),
            "minecraft:no_such_damage",
        ),
        (
            "/sound_settings/hit_sound",
            serde_json::json!("minecraft:no_such_sound"),
            "minecraft:no_such_sound",
        ),
        (
            "/attribute_modifiers/0/operation",
            serde_json::json!("multiply"),
            "multiply",
        ),
        ("/bogus", serde_json::json!(1), "bogus"),
    ];
    for (pointer, value, expected) in refused {
        let mut json = valid.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        json.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), value);
        let message = test_registries().scope(|| {
            serde_json::from_value::<SulfurCubeArchetype>(json)
                .expect_err(pointer)
                .to_string()
        });
        assert!(message.contains(expected), "{pointer}: {message}");
    }
}

#[test]
fn every_enchantment_provider_type_round_trips() {
    assert_samples_round_trip::<EnchantmentProvider>(
        "minecraft:enchantment_provider_type",
        &[
            (
                "minecraft:by_cost",
                r##"{"type":"minecraft:by_cost","enchantments":"#minecraft:on_mob_spawn_equipment",
                    "cost":{"type":"minecraft:uniform","min_inclusive":5,"max_inclusive":20}}"##,
            ),
            (
                "minecraft:by_cost_with_difficulty",
                r#"{"type":"minecraft:by_cost_with_difficulty",
                    "enchantments":["minecraft:sharpness","minecraft:smite"],
                    "min_cost":5,"max_cost_span":17}"#,
            ),
            (
                "minecraft:single",
                r#"{"type":"minecraft:single","enchantment":"minecraft:silk_touch","level":1}"#,
            ),
        ],
    );

    for json in [
        r#"{"type":"minecraft:by_cost","enchantments":"minecraft:sharpness","cost":7}"#,
        r##"{"type":"minecraft:by_cost_with_difficulty",
            "enchantments":"#minecraft:on_mob_spawn_equipment","min_cost":1,"max_cost_span":0}"##,
        r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness",
            "level":{"type":"minecraft:uniform","min_inclusive":1,"max_inclusive":3}}"#,
    ] {
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(round_trip::<EnchantmentProvider>(json), read, "{json}");
    }
}

#[test]
fn a_provider_the_game_refuses_fails_to_parse() {
    for (json, expected) in [
        (
            r#"{"type":"minecraft:by_cost_with_difficulty","enchantments":"minecraft:sharpness",
                "min_cost":0,"max_cost_span":1}"#,
            "[1;10000]",
        ),
        (
            r#"{"type":"minecraft:by_cost_with_difficulty","enchantments":"minecraft:sharpness",
                "min_cost":1,"max_cost_span":10001}"#,
            "[0;10000]",
        ),
        (
            r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness"}"#,
            "level",
        ),
        (
            r#"{"type":"minecraft:nowhere","enchantment":"minecraft:sharpness","level":1}"#,
            "minecraft:nowhere",
        ),
        (
            r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness","level":1,"bogus":1}"#,
            "bogus",
        ),
    ] {
        let message = test_registries().scope(|| {
            serde_json::from_str::<EnchantmentProvider>(json)
                .expect_err(json)
                .to_string()
        });
        assert!(message.contains(expected), "{json}: {message}");
    }
}

fn trade_with_modifier(modifier: &str) -> String {
    format!(
        r#"{{"gives":{{"id":"minecraft:emerald"}},"wants":{{"id":"minecraft:stick"}},
        "given_item_modifier":{modifier}}}"#
    )
}

#[test]
fn trade_forms_beyond_the_shipped_files_round_trip() {
    let trade = |overrides: serde_json::Value| {
        let mut trade = serde_json::json!({
            "gives": {"id": "minecraft:emerald"},
            "wants": {"id": "minecraft:stick"},
        });
        trade
            .as_object_mut()
            .unwrap()
            .extend(overrides.as_object().unwrap().clone());
        trade
    };
    let cases = [
        (trade(serde_json::json!({})), None),
        (
            trade(serde_json::json!({"gives": "minecraft:emerald", "max_uses": 4, "xp": 1})),
            Some(trade(serde_json::json!({}))),
        ),
        (
            trade(serde_json::json!({
                "max_uses": {"type": "minecraft:constant", "value": 3},
                "xp": {"type": "minecraft:uniform", "min": 1,
                    "max": {"type": "minecraft:binomial", "n": 4, "p": 0.5}},
                "reputation_discount": {"type": "minecraft:constant", "value": 0.25},
            })),
            Some(trade(serde_json::json!({
                "max_uses": 3,
                "xp": {"type": "minecraft:uniform", "min": 1,
                    "max": {"type": "minecraft:binomial", "n": 4, "p": 0.5}},
                "reputation_discount": 0.25,
            }))),
        ),
        (
            trade(serde_json::json!({"given_item_modifier": [
                {"type": "minecraft:discard"},
                {"type": "minecraft:set_potion", "id": "minecraft:water"},
            ]})),
            None,
        ),
        (
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:filtered",
                "condition": {"type": "minecraft:inverted",
                    "term": {"type": "minecraft:weather_check", "raining": true}},
                "item_filter": {"items": "minecraft:bow"},
                "on_pass": [{"type": "minecraft:set_random_potion"}],
                "on_fail": {"type": "minecraft:discard"},
            }})),
            None,
        ),
        (
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:exploration_map",
                "destination": "#minecraft:on_woodland_mansion_maps",
                "zoom": 2, "search_radius": 50, "skip_existing_chunks": true,
            }})),
            Some(trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:exploration_map",
                "destination": "#minecraft:on_woodland_mansion_maps",
            }}))),
        ),
    ];
    for (input, expected) in cases {
        let expected = expected.unwrap_or_else(|| input.clone());
        assert_eq!(round_trip::<VillagerTrade>(&input.to_string()), expected);
    }
}

#[test]
fn trade_values_the_game_refuses_fail_to_parse() {
    let wants =
        |id: &str| format!(r#"{{"gives":{{"id":"minecraft:emerald"}},"wants":{{"id":"{id}"}}}}"#);
    let modifier = |modifier: &str| trade_with_modifier(modifier);
    let cases = [
        (wants("minecraft:air"), "minecraft:air"),
        (
            r#"{"gives":{"id":"minecraft:emerald","count":0},"wants":{"id":"minecraft:stick"}}"#
                .to_owned(),
            "[1;99]",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},"bogus":1}"#
                .to_owned(),
            "bogus",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "max_uses":{"type":"minecraft:abs","value":1}}"#
                .to_owned(),
            "minecraft:abs",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "reputation_discount":{"type":"minecraft:uniform","min":0.0,"max":1.0}}"#
                .to_owned(),
            "minecraft:uniform",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "max_uses":3000000000}"#
                .to_owned(),
            "3000000000",
        ),
        (
            modifier(r#""minecraft:some_item_modifier""#),
            "minecraft:some_item_modifier",
        ),
        (
            modifier(
                r#"{"type":"minecraft:set_stew_effect","effects":[
                {"type":"minecraft:poison","duration":5},{"type":"minecraft:poison","duration":6}]}"#,
            ),
            "duplicate mob effect",
        ),
        (
            modifier(
                r#"{"type":"minecraft:set_stew_effect","effects":[
                {"type":"minecraft:no_such_effect","duration":5}]}"#,
            ),
            "minecraft:no_such_effect",
        ),
        (
            modifier(r#"{"type":"minecraft:set_potion","id":"minecraft:no_such_potion"}"#),
            "minecraft:no_such_potion",
        ),
        (
            modifier(
                r##"{"type":"minecraft:enchant_randomly","options":"#minecraft:no_such_tag"}"##,
            ),
            "minecraft:no_such_tag",
        ),
        (
            modifier(
                r##"{"type":"minecraft:exploration_map","destination":"#minecraft:no_such_tag"}"##,
            ),
            "minecraft:no_such_tag",
        ),
        (
            modifier(r#"{"type":"minecraft:discard","bogus":1}"#),
            "bogus",
        ),
    ];
    for (json, expected) in cases {
        let message = test_registries().scope(|| {
            serde_json::from_str::<VillagerTrade>(&json)
                .expect_err(&json)
                .to_string()
        });
        assert!(message.contains(expected), "{json}: {message}");
    }
}

#[test]
fn every_built_in_file_reads_through_the_layered_file_source() {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(
            &AssetPlugin::default().file_path,
            mcrs_minecraft_worldgen_builtin::asset,
        ),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    let source = app
        .world()
        .resource::<AssetServer>()
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing")
        .reader();

    for directory in [
        "minecraft/worldgen/template_pool",
        "minecraft/worldgen/noise",
        "minecraft/worldgen/density_function",
        "minecraft/worldgen/noise_settings",
    ] {
        let paths = mcrs_minecraft_worldgen_builtin::paths(directory);
        assert!(!paths.is_empty(), "{directory}");
        for path in paths {
            bevy_tasks::block_on(read_whole(source, Path::new(&path)))
                .unwrap_or_else(|error| panic!("{path}: {error}"));
        }
    }
}

fn timeline_file(clock: &str, markers: &str) -> String {
    format!(r#"{{"clock":"{clock}","period_ticks":24000,"time_markers":{markers}}}"#)
}

fn load_shipped_and(timelines: &[(&str, String)]) -> Result<RegistrySet, String> {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(
            &AssetPlugin::default().file_path,
            mcrs_minecraft_worldgen_builtin::asset,
        ),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    let asset_server = app.world().resource::<AssetServer>().clone();

    let mut packs = read_packs(&asset_server, &WORLD, &STATICS);
    packs.push(Pack {
        name: "extra".to_owned(),
        files: timelines
            .iter()
            .map(|(name, json)| PackFile {
                path: format!("test/timeline/{name}.json"),
                bytes: Some(json.clone().into_bytes()),
            })
            .collect(),
        built: Vec::new(),
    });
    WORLD
        .load(&STATICS, &packs)
        .map_err(|report| report.to_string())
}

#[test]
fn a_time_marker_defined_twice_for_one_clock_fails_the_load() {
    let refused = load_shipped_and(&[
        (
            "first",
            timeline_file("minecraft:overworld", r#"{"test:marker":1000}"#),
        ),
        (
            "second",
            timeline_file("minecraft:overworld", r#"{"test:marker":2000}"#),
        ),
    ])
    .err()
    .expect("a marker defined by two timelines of one clock is refused");

    let (header, line) = refused.split_once('\n').expect("a header and one line");
    assert_eq!(header, "registry load failed: 1 errors in 1 registries");
    let key = "minecraft:timeline/test:second (test/timeline/second.json): ";
    let message = line
        .strip_prefix(key)
        .unwrap_or_else(|| panic!("the line is keyed by the second timeline: {line}"));
    assert!(message.contains("test:marker"), "{message}");
    assert!(message.contains("minecraft:overworld"), "{message}");
    assert!(message.contains("more than once"), "{message}");
}

#[test]
fn a_marker_reused_on_two_clocks_loads() {
    let set = load_shipped_and(&[
        (
            "first",
            timeline_file("minecraft:overworld", r#"{"test:marker":1000}"#),
        ),
        (
            "second",
            timeline_file("minecraft:the_end", r#"{"test:marker":2000}"#),
        ),
    ])
    .unwrap_or_else(|report| panic!("the markers are on two clocks: {report}"));

    let table = set.table("minecraft:timeline").expect("the timeline table");
    let timelines = set
        .column::<Timeline>("minecraft:timeline")
        .expect("the timelines parse");
    assert_eq!(timelines.len(), table.len());
    assert!(table.number("test:first").is_some() && table.number("test:second").is_some());
}

#[test]
fn a_timeline_naming_an_unknown_clock_fails() {
    let refused = load_shipped_and(&[("lost", timeline_file("minecraft:nowhere", "{}"))])
        .err()
        .expect("a clock the registry does not hold is refused");

    assert!(refused.contains("minecraft:world_clock"), "{refused}");
    assert!(refused.contains("minecraft:nowhere"), "{refused}");
    assert!(
        refused.contains("minecraft:timeline/test:lost (test/timeline/lost.json)"),
        "{refused}"
    );
}

fn json_names(directory: &Path, prefix: &str, names: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let file = entry.file_name().into_string().unwrap();
        if entry.path().is_dir() {
            json_names(&entry.path(), &format!("{prefix}{file}/"), names);
        } else if let Some(stem) = file.strip_suffix(".json") {
            names.insert(format!("minecraft:{prefix}{stem}"));
        }
    }
}

#[test]
fn trial_spawners_have_names_and_no_values() {
    for loot in [
        "minecraft:loot_table",
        "minecraft:predicate",
        "minecraft:item_modifier",
    ] {
        assert!(
            !WORLD.declared().any(|registry| registry.as_str() == loot),
            "{loot} is declared"
        );
        assert!(
            test_registries().table(loot).is_none(),
            "{loot} has a table"
        );
    }

    let mut shipped = BTreeSet::new();
    json_names(&assets().join("minecraft/trial_spawner"), "", &mut shipped);
    let table = test_registries()
        .table("minecraft:trial_spawner")
        .expect("trial spawner table");
    let named: BTreeSet<String> = table.names().iter().map(|name| name.to_string()).collect();
    assert!(!named.is_empty());
    assert_eq!(named, shipped);
}

#[test]
fn the_worldgen_tables_hold_every_loaded_carver_by_id() {
    let app = crate::common::run_to_playing();
    let set = app.world().resource::<RegistrySet>();
    let carvers = set
        .registry::<CarverConfig>()
        .expect("the carver registry is declared");
    let tables = app
        .world()
        .resource::<mcrs_minecraft_worldgen::tables::WorldgenTables>();

    assert!(carvers.by_name("minecraft:beta_cave").is_some());
    for id in carvers.ids() {
        let name = carvers.name(id).unwrap();
        assert!(
            tables.carvers.get(id).is_some_and(Option::is_some),
            "{name} has no value in the carver table"
        );
    }
}

fn overworld_dimension_type_with(
    changes: &[(&str, Option<&str>)],
) -> Result<DimensionTypeFile, String> {
    let mut file: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets().join("minecraft/dimension_type/overworld.json")).unwrap(),
    )
    .unwrap();
    for (field, value) in changes {
        match value {
            Some(json) => file[*field] = serde_json::from_str(json).unwrap(),
            None => {
                file.as_object_mut().unwrap().remove(*field);
            }
        }
    }
    test_registries().scope(|| serde_json::from_value(file).map_err(|e| e.to_string()))
}

fn tag_in<R: mcrs_minecraft_registry::Registered>(set: &RegistrySet, name: &str) -> TagId<R> {
    set.tags::<R>()
        .unwrap_or_else(|| panic!("{} has loaded tags", R::REGISTRY))
        .get(&TagKey::<R, _>::from_location(
            ResourceLocation::read(name).unwrap(),
        ))
        .unwrap_or_else(|| panic!("{name} is a loaded tag"))
}

#[test]
fn a_dimension_type_reads_its_holder_fields_as_vanilla_does() {
    let set = test_registries();
    let stone = set
        .registry::<Block>()
        .unwrap()
        .by_name("minecraft:stone")
        .unwrap();
    let overworld_clock = set
        .registry::<WorldClock>()
        .unwrap()
        .get(&mcrs_minecraft_environment::keys::world_clock::OVERWORLD)
        .unwrap();

    let infiniburn: [(&str, HolderSet<Block>); 3] = [
        (
            r##""#minecraft:infiniburn_overworld""##,
            HolderSet::Named(tag_in::<Block>(set, "minecraft:infiniburn_overworld")),
        ),
        (r#""minecraft:stone""#, HolderSet::One(stone)),
        (r#"["minecraft:stone"]"#, HolderSet::List(Box::new([stone]))),
    ];
    for (json, expected) in infiniburn {
        let read = overworld_dimension_type_with(&[("infiniburn", Some(json))])
            .unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(read.infiniburn, expected, "{json}");
    }

    let absent = overworld_dimension_type_with(&[("timelines", None), ("default_clock", None)])
        .expect("a dimension type without timelines or a clock reads");
    assert_eq!(absent.timelines, HolderSet::default());
    assert_eq!(absent.default_clock, None);

    let shipped = overworld_dimension_type_with(&[]).expect("the shipped overworld reads");
    assert_eq!(
        shipped.timelines,
        HolderSet::Named(tag_in::<Timeline>(set, "minecraft:in_overworld"))
    );
    assert_eq!(shipped.default_clock, Some(overworld_clock));

    for (field, json, named) in [
        (
            "infiniburn",
            r##""#minecraft:no_such_tag""##,
            "minecraft:no_such_tag",
        ),
        (
            "infiniburn",
            r#""minecraft:no_such_block""#,
            "minecraft:no_such_block",
        ),
        (
            "infiniburn",
            r#"["minecraft:no_such_block"]"#,
            "minecraft:no_such_block",
        ),
        (
            "timelines",
            r##""#minecraft:no_such_tag""##,
            "minecraft:no_such_tag",
        ),
        (
            "timelines",
            r#"["minecraft:no_such_timeline"]"#,
            "minecraft:no_such_timeline",
        ),
        (
            "default_clock",
            r#""minecraft:no_such_clock""#,
            "minecraft:no_such_clock",
        ),
    ] {
        let message = overworld_dimension_type_with(&[(field, Some(json))])
            .expect_err(&format!("{field}: {json} reads"));
        assert!(message.contains(named), "{field}: {message}");
    }
}

fn the_loaded_presets() -> (
    mcrs_minecraft_registry::Registry<mcrs_minecraft_world::worldgen::world_preset::WorldPreset>,
    mcrs_minecraft_registry::Entries<
        mcrs_minecraft_world::worldgen::world_preset::WorldPreset,
        WorldPreset,
    >,
) {
    let set = test_registries();
    (
        set.registry::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset>()
            .expect("world presets are a declared registry"),
        set.entries::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset, WorldPreset>()
            .expect("the loader parses world presets"),
    )
}

#[test]
fn every_preset_parses_in_registry_context() {
    let set = test_registries();
    let (names, presets) = the_loaded_presets();
    let preset = |name: &str| {
        &presets[names
            .by_name(name)
            .unwrap_or_else(|| panic!("no preset {name}"))]
    };
    let overworld = |name: &str| &preset(name).dimensions["minecraft:overworld"].generator;
    let id_in = |registry: &str, name: &str| {
        set.table(registry)
            .and_then(|table| table.number(name))
            .unwrap_or_else(|| panic!("{registry} has no {name}"))
    };

    let ChunkGenerator::Noise(normal) = overworld("minecraft:normal") else {
        panic!("the normal overworld is a noise generator");
    };
    let BiomeSource::MultiNoise(source) = &normal.biome_source else {
        panic!("the normal overworld uses a multi-noise biome source");
    };
    assert_eq!(
        source.preset.map(|list| list.number()),
        Some(id_in(
            "minecraft:worldgen/multi_noise_biome_source_parameter_list",
            "minecraft:overworld"
        ))
    );
    assert_eq!(
        normal.settings.number(),
        id_in("minecraft:worldgen/noise_settings", "minecraft:overworld")
    );

    let ChunkGenerator::Noise(single) = overworld("minecraft:single_biome_surface") else {
        panic!("single_biome_surface is a noise generator");
    };
    assert_eq!(
        single.biome_source,
        BiomeSource::Fixed {
            biome: set
                .registry::<Biome>()
                .unwrap()
                .get(&mcrs_minecraft_biome::keys::biome::PLAINS)
                .unwrap()
        }
    );

    let ChunkGenerator::Flat(flat) = overworld("minecraft:flat") else {
        panic!("expected a flat generator");
    };
    assert_eq!(preset("minecraft:flat").dimensions.len(), 3);
    assert_eq!(flat.settings.layers.len(), 3);
    assert_eq!(flat.settings.structure_overrides.len(), 2);

    assert_eq!(
        overworld("minecraft:debug_all_block_states"),
        &ChunkGenerator::Debug
    );

    for id in names.ids() {
        let name = names.name(id).unwrap();
        let written = set
            .scope(|| serde_json::to_string(&presets[id]))
            .unwrap_or_else(|e| panic!("{name} does not encode: {e}"));
        let read: WorldPreset = set
            .scope(|| serde_json::from_str(&written))
            .unwrap_or_else(|e| panic!("{name} does not read back: {e}"));
        assert_eq!(
            read, presets[id],
            "{name} changed across an encode and a parse"
        );
    }
}

#[test]
fn a_preset_reads_its_dimensions_as_a_map_by_key() {
    let set = test_registries();
    let (names, presets) = the_loaded_presets();
    let preset = |name: &str| &presets[names.by_name(name).unwrap()];

    let beta = preset("minecraft:beta");
    assert_eq!(
        beta.dimensions
            .keys()
            .map(|key| key.as_str())
            .collect::<Vec<_>>(),
        ["minecraft:overworld"]
    );
    let beta_type = set
        .registry::<DimensionType>()
        .unwrap()
        .by_name("minecraft:beta")
        .expect("the beta pack ships the beta dimension type");
    assert_eq!(
        beta.dimensions["minecraft:overworld"].dimension_type,
        beta_type
    );

    let shipped: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets().join("minecraft/worldgen/world_preset/normal.json")).unwrap(),
    )
    .unwrap();
    let listing = |order: [&str; 3]| {
        let entries: Vec<String> = order
            .iter()
            .map(|key| format!("\"{key}\":{}", shipped["dimensions"][key]))
            .collect();
        format!(r#"{{"dimensions":{{{}}}}}"#, entries.join(","))
    };
    let parse = |text: String| -> WorldPreset {
        set.scope(|| serde_json::from_str(&text))
            .unwrap_or_else(|e| panic!("{e}: {text}"))
    };
    let forward = parse(listing([
        "minecraft:overworld",
        "minecraft:the_nether",
        "minecraft:the_end",
    ]));
    let backward = parse(listing([
        "minecraft:the_end",
        "minecraft:the_nether",
        "minecraft:overworld",
    ]));
    assert_eq!(forward, backward);
    assert_eq!(&forward, preset("minecraft:normal"));
}

#[test]
fn an_inline_dimension_type_is_refused() {
    let inline: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets().join("minecraft/dimension_type/overworld.json")).unwrap(),
    )
    .unwrap();
    let text = format!(
        r#"{{"dimensions":{{"minecraft:overworld":{{"type":{inline},"generator":{{"type":"minecraft:debug"}}}}}}}}"#
    );
    let error = test_registries()
        .scope(|| serde_json::from_str::<WorldPreset>(&text))
        .expect_err("a dimension type written out in the preset is refused")
        .to_string();
    assert!(error.contains("minecraft:dimension_type"), "{error}");
}

#[test]
fn an_empty_preset_object_names_dimensions() {
    let refused = refused_by_the_loader("worldgen/world_preset", "empty", "{}");
    let line = refused
        .lines()
        .find(|line| line.contains("minecraft:empty"))
        .unwrap_or_else(|| panic!("no line names the preset: {refused}"));
    assert!(line.contains("dimensions"), "{line}");
}

#[test]
fn a_preset_naming_something_the_registries_lack_fails_the_load() {
    let cases = [
        (
            "minecraft:dimension_type",
            r#"{"type":"test:no_such_name","generator":{"type":"minecraft:debug"}}"#,
        ),
        (
            "minecraft:worldgen/noise_settings",
            r#"{"generator":{"type":"minecraft:noise","settings":"test:no_such_name","biome_source":{"type":"minecraft:the_end"}},"type":"minecraft:overworld"}"#,
        ),
        (
            "minecraft:worldgen/biome",
            r#"{"generator":{"type":"minecraft:noise","biome_source":{"type":"minecraft:fixed","biome":"test:no_such_name"},"settings":"minecraft:overworld"},"type":"minecraft:overworld"}"#,
        ),
        (
            "minecraft:worldgen/biome",
            r#"{"generator":{"type":"minecraft:flat","settings":{"biome":"test:no_such_name","layers":[]}},"type":"minecraft:overworld"}"#,
        ),
        (
            "minecraft:worldgen/multi_noise_biome_source_parameter_list",
            r#"{"generator":{"type":"minecraft:noise","biome_source":{"type":"minecraft:multi_noise","preset":"test:no_such_name"},"settings":"minecraft:overworld"},"type":"minecraft:overworld"}"#,
        ),
    ];
    for (registry, entry) in cases {
        let preset = format!(r#"{{"dimensions":{{"minecraft:overworld":{entry}}}}}"#);
        let refused = refused_by_the_loader("worldgen/world_preset", "test_preset", &preset);
        let line = refused
            .lines()
            .find(|line| line.contains("test_preset"))
            .unwrap_or_else(|| panic!("{registry}: no line names the preset: {refused}"));
        assert!(line.contains(registry), "{registry}: {line}");
        assert!(line.contains("test:no_such_name"), "{registry}: {line}");
    }
}

#[test]
fn an_item_and_a_block_of_one_name_keep_their_own_numbers() {
    let set = test_registries();
    let items = set.registry::<keys::Item>().unwrap();
    let blocks = set.registry::<keys::Block>().unwrap();
    assert_eq!(
        items.by_name("minecraft:stone"),
        Some(keys::item::STONE.id())
    );
    assert_eq!(
        blocks.by_name("minecraft:stone"),
        Some(keys::block::STONE.id())
    );

    let mut differing = 0;
    for item in items.ids() {
        let name = items.name(item).unwrap().as_str();
        let Some(block) = blocks.by_name(name) else {
            continue;
        };
        assert_eq!(keys::item::find(name).unwrap().id(), item, "{name}");
        assert_eq!(keys::block::find(name).unwrap().id(), block, "{name}");
        differing += usize::from(item.number() != block.number());
    }
    assert!(differing > 0, "no shared name is numbered differently");
}
