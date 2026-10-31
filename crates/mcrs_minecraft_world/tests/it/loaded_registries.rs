use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_resource::Registered;
use mcrs_minecraft_resource::ResourceKey;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_app::{App, Plugin, TaskPoolPlugin};
use bevy_asset::io::memory::{Dir, MemoryAssetReader};
use bevy_asset::io::{AssetReaderError, AssetSourceBuilder, AssetSourceId};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PackLayers, layered_file_source, layered_reader};
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_datapack::{Level, PackDiagnostic, StackedPack, VANILLA_PACK};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine_registries::SyncedRegistries;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::dialog::{Action, Dialog, DialogBody, Input};
use mcrs_minecraft_item::{BannerPattern, InstrumentValue, PaintingVariantValue};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::{
    CurrentRegistries, Declaration, Diagnostic, IdSet, NetworkSnapshot, Registries,
    RegistryAdditions, Resolve, Resolver, Stage, insert_entry,
};
use mcrs_minecraft_resource::{
    Id, RegistryKey, RegistryValue, ResourceLocation, TagId, TagKey, rl,
};
use mcrs_minecraft_world::registry_load::{
    RegistryLoadError, load_registries, sent_by, test_network, test_registries,
};
use mcrs_minecraft_world::sulfur_cube_archetype::SulfurCubeArchetype;
use mcrs_minecraft_world::variant::{WolfVariantFile, spawn_selectors};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_worldgen_testing::{corpus_path, packs, vanilla_dir};

use crate::common::{
    WithFiles, assets, datapack_report, declared_registries, loaded_names, plugin_app, read_named,
    tag_members, write_named,
};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};

#[derive(serde::Deserialize)]
struct ReportedRegistry {
    entries: BTreeMap<String, ReportedEntry>,
}

#[derive(serde::Deserialize)]
struct ReportedEntry {
    protocol_id: usize,
}

#[test]
fn the_loaded_static_registries_number_their_entries_as_the_report() {
    let report: BTreeMap<String, ReportedRegistry> = serde_json::from_slice(
        &std::fs::read(assets().join("mcrs/reports/registries.json")).unwrap(),
    )
    .unwrap();
    assert!(!report.is_empty());
    for (registry, reported) in report {
        let mut expected = vec![String::new(); reported.entries.len()];
        for (name, entry) in reported.entries {
            expected[entry.protocol_id] = name;
        }
        assert_eq!(
            loaded_names(test_registries(), &registry),
            expected,
            "{registry}"
        );
    }
}

fn load_text(files: &[(&str, &str)]) -> String {
    WithFiles::test_pack(files)
        .load()
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
}

fn errors_of<T>(loaded: Result<T, RegistryLoadError>) -> Vec<Diagnostic> {
    match loaded {
        Ok(_) => Vec::new(),
        Err(RegistryLoadError::Load(diagnostics)) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.level() == Level::Error)
            .cloned()
            .collect(),
        Err(error) => panic!("the load stops before the loader runs: {error}"),
    }
}

fn refused_by_the_loader(registry: &str, name: &str, json: &str) -> String {
    load_text(&[(&format!("minecraft/{registry}/{name}.json"), json)])
}

/// One load for many refused files: each case is written as its own entry, and its text is
/// what the diagnostics say about that entry.
fn refused_each(cases: &[(&str, String)]) -> Vec<(String, String)> {
    let names: Vec<String> = (0..cases.len()).map(|at| format!("odd{at:03}")).collect();
    let paths: Vec<String> = cases
        .iter()
        .zip(&names)
        .map(|((registry, _), name)| format!("minecraft/{registry}/{name}.json"))
        .collect();
    let files: Vec<(&str, &str)> = paths
        .iter()
        .zip(cases)
        .map(|(path, (_, json))| (path.as_str(), json.as_str()))
        .collect();
    let diagnostics: Vec<String> = errors_of(WithFiles::test_pack(&files).load())
        .iter()
        .map(ToString::to_string)
        .collect();
    names
        .into_iter()
        .map(|name| {
            let entry = format!("minecraft:{name}");
            let text = diagnostics
                .iter()
                .filter(|text| text.contains(&entry))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            (name, text)
        })
        .collect()
}

fn instrument(sound: &str) -> String {
    format!(
        r#"{{"sound_event":"{sound}","use_duration":7.0,"range":256.0,
        "description":{{"translate":"instrument.minecraft.ponder_goat_horn"}}}}"#
    )
}

fn every_registry() -> BTreeSet<String> {
    let path = assets().join("mcrs/reports/registries.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let report: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    report
        .into_keys()
        .chain(declared_registries(false))
        .chain(declared_registries(true))
        .collect()
}

#[test]
fn every_registry_of_the_reports_is_loaded() {
    let report = datapack_report();
    let set = test_registries();
    for registry in every_registry() {
        assert!(
            set.names_of(&ResourceLocation::read(&registry).unwrap())
                .is_some(),
            "{registry} is not loaded"
        );
    }
    for (registry, flags) in &report.registries {
        let directory = vanilla_dir().join(registry.trim_start_matches("minecraft:"));
        if directory.is_dir() {
            assert!(
                flags.elements,
                "{registry} has files in the vanilla data pack and no elements"
            );
        }
    }
}

#[test]
fn every_shipped_and_builtin_biome_has_a_name_from_the_loader() {
    let names: BTreeSet<String> = loaded_names(test_registries(), "minecraft:worldgen/biome")
        .into_iter()
        .collect();
    let built = mcrs_minecraft_worldgen_builtin::biomes().unwrap();
    assert!(!built.is_empty());
    let files: Vec<String> = std::iter::once(vanilla_dir().join("worldgen/biome"))
        .chain(
            packs()
                .into_iter()
                .map(|root| root.join("minecraft/worldgen/biome")),
        )
        .filter_map(|directory| std::fs::read_dir(directory).ok())
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

fn from_pack<V: RegistryValue + 'static>(set: &Registries, pack: &str) -> Vec<ResourceLocation> {
    let registry = set
        .registry::<V>()
        .unwrap_or_else(|| panic!("{} is not loaded", std::any::type_name::<V>()));
    registry
        .iter()
        .filter(|(id, _, _)| set.origin(*id).map(StackedPack::id) == Some(pack))
        .map(|(_, name, _)| name.clone())
        .collect()
}

fn pack_of<V: RegistryValue + 'static>(set: &Registries, name: &str) -> Option<String> {
    let registry = set.registry::<V>()?;
    let id = registry.by_name(&ResourceLocation::read(name).ok()?)?;
    set.origin(id).map(|pack| pack.id().to_owned())
}

fn key_of<V: RegistryValue>() -> ResourceLocation {
    <V::Registry as Registered>::REGISTRY.location().into()
}

#[test]
fn the_vanilla_pack_loads_without_world_generation_and_names_its_registries_only() {
    let source = WithFiles::over_assets().hiding("mcrs/datapacks/beta");
    let (registries, _) = load_registries(&source, RegistryAdditions::default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(registries.registry::<BiomeFile>().is_some());
    assert!(registries.registry::<Feature>().is_none());
    let placed = registries
        .names_of(&key_of::<PlacedFeature>())
        .expect("the placed features are declared by their names");
    assert!(!placed.is_empty());
    assert!(
        registries
            .names_of(&rl!("minecraft:worldgen/feature_type"))
            .is_none()
    );
}

#[test]
fn vanilla_biomes_are_built_entries_not_files() {
    let vanilla = from_pack::<BiomeFile>(test_registries(), VANILLA_PACK);
    assert_eq!(vanilla.len(), 68);
    assert!(
        mcrs_minecraft_worldgen_builtin::asset("minecraft/worldgen/biome/plains.json").is_none(),
        "a built biome is served as a file"
    );
}

#[test]
fn vanilla_noise_settings_are_built_entries_not_files() {
    let set = test_registries();
    type FromPack = fn(&Registries, &str) -> Vec<ResourceLocation>;
    let cases: [(&str, FromPack, usize, usize); 3] = [
        ("minecraft:worldgen/noise", from_pack::<NoiseParam>, 65, 5),
        (
            "minecraft:worldgen/density_function",
            from_pack::<DensityFunctionHolder>,
            55,
            10,
        ),
        (
            "minecraft:worldgen/noise_settings",
            from_pack::<NoiseGeneratorSettings>,
            7,
            1,
        ),
    ];
    for (registry, from, vanilla, beta) in cases {
        assert_eq!(
            (from(set, VANILLA_PACK).len(), from(set, "file/beta").len()),
            (vanilla, beta),
            "{registry}"
        );
    }
    assert!(
        mcrs_minecraft_worldgen_builtin::asset("minecraft/worldgen/noise_settings/overworld.json")
            .is_none(),
        "built noise settings are served as a file"
    );
}

#[test]
fn vanilla_template_pools_are_built_entries_not_files() {
    let set = test_registries();
    let pools = set
        .registry::<TemplatePool>()
        .expect("the loader parses template pools");
    assert_eq!(pools.len(), 245);
    assert_eq!(
        from_pack::<TemplatePool>(set, VANILLA_PACK).len(),
        pools.len(),
        "a template pool comes from a pack other than the vanilla one"
    );
    assert!(
        mcrs_minecraft_worldgen_builtin::asset("minecraft/worldgen/template_pool/empty.json")
            .is_none(),
        "a built template pool is served as a file"
    );
}

#[test]
fn the_fallback_serves_only_structure_templates() {
    let set = test_registries();
    let fallback = layered_reader(
        Box::new(MemoryAssetReader {
            root: Dir::default(),
        }),
        &crate::common::pack_layering(),
    );

    let mut json_paths = 0;
    let built: [(ResourceLocation, Vec<ResourceLocation>); 5] = [
        (
            key_of::<BiomeFile>(),
            from_pack::<BiomeFile>(set, VANILLA_PACK),
        ),
        (
            key_of::<DensityFunctionHolder>(),
            from_pack::<DensityFunctionHolder>(set, VANILLA_PACK),
        ),
        (
            key_of::<NoiseParam>(),
            from_pack::<NoiseParam>(set, VANILLA_PACK),
        ),
        (
            key_of::<NoiseGeneratorSettings>(),
            from_pack::<NoiseGeneratorSettings>(set, VANILLA_PACK),
        ),
        (
            key_of::<TemplatePool>(),
            from_pack::<TemplatePool>(set, VANILLA_PACK),
        ),
    ];
    for (registry, names) in &built {
        for name in names {
            let path = format!(
                "{}/{}/{}.json",
                name.namespace(),
                registry.path(),
                name.path()
            );
            assert!(
                mcrs_minecraft_worldgen_builtin::asset(&path).is_none(),
                "{path} is answered from built-in code"
            );
            assert!(
                matches!(
                    bevy_tasks::block_on(read_whole(&*fallback, Path::new(&path))),
                    Err(AssetReaderError::NotFound(_))
                ),
                "{path} is read through the fallback"
            );
            json_paths += 1;
        }
    }
    assert_eq!(json_paths, 68 + 65 + 55 + 7 + 245);

    let template = mcrs_minecraft_worldgen_builtin::template_paths("minecraft/structure")
        .into_iter()
        .next()
        .expect("the code builds structure templates");
    let bytes = bevy_tasks::block_on(read_whole(&*fallback, Path::new(&template)))
        .unwrap_or_else(|error| panic!("{template}: {error}"));
    assert!(!bytes.is_empty());
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
fn a_biome_placing_a_feature_outside_the_allowed_range_fails_the_load() {
    let in_square = r#"{"type":"minecraft:in_square"}"#;
    let swing = r#"{"type":"minecraft:uniform","min_inclusive":-16,"max_inclusive":16}"#;
    let placed = |placement: String| {
        format!(r#"{{"feature":"minecraft:test_feature","placement":[{placement}]}}"#)
    };
    let far = placed(format!("{in_square},{in_square},{in_square}"));
    let edge = placed(format!(
        r#"{in_square},{{"type":"minecraft:offset","x":{swing},"y":0,"z":{swing}}}"#
    ));
    let biome = r##"{"temperature":0.5,"downfall":0.5,"has_precipitation":true,"effects":{"water_color":"#3f76e4"},
        "features":[["minecraft:test_far","minecraft:test_edge"]]}"##;
    let refused = load_text(&[
        (
            "minecraft/worldgen/feature/test_feature.json",
            r#"{"type":"minecraft:no_op","config":{}}"#,
        ),
        ("minecraft/worldgen/placed_feature/test_far.json", &far),
        ("minecraft/worldgen/placed_feature/test_edge.json", &edge),
        ("minecraft/worldgen/biome/test_biome.json", biome),
    ]);
    let line = refused
        .lines()
        .find(|line| line.contains("test_biome"))
        .unwrap_or_else(|| panic!("no line names the biome: {refused}"));
    assert!(
        line.contains("cover too large domain in XZ plane, must be at most [-16, 31]"),
        "{line}"
    );
    assert!(
        line.contains("test_far (features[0][0]) has [0, 45]"),
        "{line}"
    );
    assert!(!line.contains("test_edge"), "{line}");
}

#[test]
fn every_banner_pattern_is_named_after_its_asset() {
    let patterns = test_registries()
        .registry::<BannerPattern>()
        .expect("banner patterns are parsed by the loader");
    assert!(!patterns.is_empty());
    for (_, name, pattern) in patterns.iter() {
        assert_eq!(&pattern.asset_id.to_string(), &name.to_string());
    }
}

#[test]
fn a_local_entry_comes_from_its_pack() {
    let set = test_registries();
    type PackOf = fn(&Registries, &str) -> Option<String>;
    let dimension_type: PackOf = pack_of::<DimensionTypeFile>;
    let preset: PackOf = pack_of::<WorldPreset>;
    let noise_settings: PackOf = pack_of::<NoiseGeneratorSettings>;
    let density_function: PackOf = pack_of::<DensityFunctionHolder>;
    let noise: PackOf = pack_of::<NoiseParam>;
    for (pack_of, name, pack) in [
        (dimension_type, "minecraft:beta", "file/beta"),
        (dimension_type, "minecraft:overworld", "vanilla"),
        (preset, "minecraft:beta", "file/beta"),
        (preset, "minecraft:normal", "vanilla"),
        (noise_settings, "minecraft:beta", "file/beta"),
        (noise_settings, "minecraft:overworld", "vanilla"),
        (
            density_function,
            "minecraft:overworld/continents",
            "vanilla",
        ),
        (density_function, "minecraft:beta/temperature", "file/beta"),
        (noise, "mcrs:beta/temperature", "file/beta"),
        (noise, "minecraft:ridge", "vanilla"),
    ] {
        assert_eq!(pack_of(set, name).as_deref(), Some(pack), "{name}");
    }
}

#[test]
fn packs_follow_vanilla_in_name_order() {
    let pattern =
        |name: &str| format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#);
    let (registries, _) = WithFiles::over_assets()
        .with("minecraft/banner_pattern/a.json", pattern("a"))
        .pack("mcrs/datapacks/zz_second")
        .with(
            "mcrs/datapacks/zz_second/minecraft/banner_pattern/b.json",
            pattern("b"),
        )
        .with(
            "mcrs/datapacks/zz_second/minecraft/banner_pattern/c.json",
            pattern("second"),
        )
        .pack("mcrs/datapacks/zz_first")
        .with(
            "mcrs/datapacks/zz_first/minecraft/banner_pattern/c.json",
            pattern("first"),
        )
        .load()
        .unwrap_or_else(|error| panic!("{error}"));
    let set = &registries;

    let patterns = set.registry::<BannerPattern>().unwrap();
    let entry = |name: &str| {
        let id = patterns
            .by_name(&ResourceLocation::read(name).unwrap())
            .unwrap_or_else(|| panic!("no {name}"));
        (
            set.origin(id).map(StackedPack::id),
            patterns.get(id).unwrap().asset_id.to_string(),
        )
    };
    assert_eq!(
        entry("minecraft:a"),
        (Some(VANILLA_PACK), "minecraft:a".to_owned())
    );
    assert_eq!(
        entry("minecraft:b"),
        (Some("file/zz_second"), "minecraft:b".to_owned())
    );
    assert_eq!(
        entry("minecraft:c"),
        (Some("file/zz_second"), "minecraft:second".to_owned())
    );

    let root = Dir::default();
    root.insert_asset_text(Path::new("minecraft/banner_pattern/a.json"), &pattern("a"));
    root.insert_asset_text(Path::new("mcrs/datapacks/extra/pack.mcmeta"), "{}");
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
    let source = asset_server.get_source(AssetSourceId::Default).unwrap();
    let bytes = bevy_tasks::block_on(read_whole(
        source.reader(),
        Path::new("minecraft/banner_pattern/b.json"),
    ))
    .expect("the layered reader reads the pack's file at its virtual path");
    assert_eq!(bytes, pattern("b").as_bytes());
}

#[test]
fn the_plugin_and_the_directory_reader_load_equal_sets() {
    let app = plugin_app(());
    let through_plugin: &Registries = app.world().resource::<CurrentRegistries>();
    let through_directory: &Registries = test_registries();

    let registries = every_registry();
    assert!(registries.len() > 100, "the corpus loads few registries");
    for registry in &registries {
        assert_eq!(
            loaded_names(through_plugin, registry),
            loaded_names(through_directory, registry),
            "{registry}"
        );
    }
    let through_plugin_sent = &app.world().resource::<SyncedRegistries>().0;
    let sent = |network: &NetworkSnapshot| -> Vec<Vec<u8>> {
        network.registry_data(false).map(<[u8]>::to_vec).collect()
    };
    assert_eq!(sent(through_plugin_sent), sent(test_network()));
    assert_eq!(through_plugin_sent.tags(), test_network().tags());
    assert!(
        through_plugin
            .known_packs()
            .eq(through_directory.known_packs())
    );
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Kit {
    size: i32,
}

impl Registered for Kit {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("t:kit"));
}

mcrs_minecraft_registries::resolve_as_is!(Kit);

struct KitPlugin;

impl Plugin for KitPlugin {
    fn build(&self, app: &mut App) {
        let mut additions = app.world_mut().get_resource_or_init::<RegistryAdditions>();
        let additions = &mut *additions;
        let files = &mut additions.files;
        files.insert("kit", "data/t/kit/a.json", r#"{"size":1}"#);
        insert_entry::<Kit>(files, "kit", &rl!("t:b").into(), Kit { size: 2 });
        files.insert(
            "kit",
            "data/t/tags/kit/all.json",
            r#"{"values":["t:a","t:b"]}"#,
        );
        additions.embedded_packs.push("kit".to_owned());
        additions
            .declarations
            .push(Declaration::data::<Kit>().synced().into());
    }
}

#[test]
fn a_plugin_registry_loads_from_its_embedded_pack_and_leaves_what_a_client_receives_unchanged() {
    let app = plugin_app(KitPlugin);
    let loaded: &Registries = app.world().resource::<CurrentRegistries>();

    let kits = loaded.registry::<Kit>().expect("the plugin declares t:kit");
    assert_eq!(**kits.values(), [Kit { size: 1 }, Kit { size: 2 }]);
    assert_eq!(loaded_names(loaded, "t:kit"), ["t:a", "t:b"]);
    assert_eq!(tag_members::<Kit>(loaded, "t:all"), ["t:a", "t:b"]);
    for registry in every_registry() {
        assert_eq!(
            loaded_names(loaded, &registry),
            loaded_names(test_registries(), &registry),
            "{registry}"
        );
    }

    let sent = &app.world().resource::<SyncedRegistries>().0;
    for known in [false, true] {
        assert!(
            (sent.registry_data(known)).eq(test_network().registry_data(known)),
            "registry data, known: {known}"
        );
    }
    assert_eq!(sent.tags(), test_network().tags());
}

#[test]
fn an_embedded_pack_overrides_vanilla_below_the_root_packs_and_is_always_sent_with_data() {
    let pattern =
        |name: &str| format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#);
    let source = WithFiles::over_assets()
        .pack("mcrs/datapacks/zz_world")
        .with(
            "mcrs/datapacks/zz_world/minecraft/banner_pattern/piglin.json",
            pattern("world"),
        );
    let embedding = |ids: &[&str]| {
        let mut additions = source.additions();
        for name in ["globe", "piglin"] {
            let path = format!("data/minecraft/banner_pattern/{name}.json");
            additions.files.insert("extra", path, pattern("embedded"));
        }
        additions.embedded_packs = ids.iter().map(|id| id.to_string()).collect();
        additions
    };

    let (registries, _) =
        load_registries(&source, embedding(&["extra"])).unwrap_or_else(|error| panic!("{error}"));
    let patterns = registries.registry::<BannerPattern>().unwrap();
    let entry = |name: &str| {
        let id = patterns
            .by_name(&ResourceLocation::read(name).unwrap())
            .unwrap();
        let origin = registries.origin(id).unwrap();
        let asset = patterns.get(id).unwrap().asset_id.to_string();
        (origin.id(), origin.known_pack().is_some(), asset)
    };
    assert_eq!(
        entry("minecraft:creeper"),
        (VANILLA_PACK, true, "minecraft:creeper".to_owned())
    );
    assert_eq!(
        entry("minecraft:globe"),
        ("extra", false, "minecraft:embedded".to_owned())
    );
    assert_eq!(
        entry("minecraft:piglin"),
        ("file/zz_world", false, "minecraft:world".to_owned())
    );
    let (_, sent) = (sent_by(&registries, true).registries.into_iter())
        .find(|(registry, _)| registry.as_str() == "minecraft:banner_pattern")
        .expect("the banner patterns are synced");
    let with_data = |name: &str| {
        (sent.iter())
            .find(|(entry, _)| entry.as_str() == name)
            .map(|(_, data)| data.is_some())
    };
    assert_eq!(with_data("minecraft:creeper"), Some(false));
    assert_eq!(with_data("minecraft:globe"), Some(true));

    for ids in [&["extra", "extra"][..], &["file/zz_world"], &[VANILLA_PACK]] {
        match load_registries(&source, embedding(ids)) {
            Err(RegistryLoadError::Declarations(message)) => {
                assert!(message.contains(ids.last().unwrap()), "{ids:?}: {message}")
            }
            Ok(_) => panic!("{ids:?}: the load succeeds"),
            Err(error) => panic!("{ids:?}: {error}"),
        }
    }
}

#[test]
fn a_pack_directory_without_a_namespace_has_no_files() {
    let (registries, diagnostics) = WithFiles::over_assets()
        .pack("mcrs/datapacks/zz_empty")
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(registries.known_packs().all(|known| known.id != "zz_empty"));
    assert!(
        !diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            Diagnostic::Pack(PackDiagnostic::UnavailablePack { pack, .. }) if pack == "file/zz_empty"
        )),
        "{:?}",
        diagnostics
    );
}

#[test]
fn an_unreadable_registry_file_is_a_report_entry_naming_its_path() {
    let refused = WithFiles::over_assets()
        .pack("mcrs/datapacks/broken")
        .unreadable("mcrs/datapacks/broken/minecraft/worldgen/biome/ghost.json")
        .load()
        .err();

    let message = refused
        .expect("a file that does not read is refused")
        .to_string();
    assert!(
        message.contains("broken/minecraft/worldgen/biome/ghost.json"),
        "{message}"
    );
}

#[test]
fn an_extra_declaration_takes_the_stage_the_report_gives_its_key_or_keeps_its_own() {
    let report = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/reports/datapack.json"
    ));
    let declarations = mcrs_minecraft_world::registries::declarations(
        report,
        vec![
            Declaration::names_only(rl!("minecraft:trade_set").into(), Stage::Reloadable),
            Declaration::names_only(rl!("test:extra").into(), Stage::Reloadable),
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let stages = |key: &str| -> Vec<Stage> {
        let key = ResourceLocation::read(key).unwrap();
        let declared = declarations
            .iter()
            .filter(|declaration| *declaration.key() == key);
        declared.map(Declaration::stage).collect()
    };
    assert_eq!(stages("minecraft:trade_set"), [Stage::World]);
    assert_eq!(stages("test:extra"), [Stage::Reloadable]);
}

fn json_at(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn a_loaded_holder_set_answers_membership_through_the_tags() {
    let set = test_registries();
    let archetypes = set
        .registry::<SulfurCubeArchetype>()
        .expect("the archetypes are parsed by the loader");
    let items = set.entries::<Item>().unwrap();
    let tags = set.tags::<Item>().unwrap();

    let mut checked = 0;
    for (_, name, archetype) in archetypes.iter() {
        let file = json_at(
            &vanilla_dir()
                .join("sulfur_cube_archetype")
                .join(format!("{}.json", name.path())),
        );
        let Some(tag) = file["items"]
            .as_str()
            .and_then(|items| items.strip_prefix('#'))
        else {
            continue;
        };
        let tag = ResourceLocation::read(tag).unwrap();
        let listed = json_at(&corpus_path(&format!(
            "{}/tags/item/{}.json",
            tag.namespace(),
            tag.path()
        )));
        let listed: Vec<&str> = listed["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        if listed.iter().any(|value| value.starts_with('#')) {
            continue;
        }
        for (id, item) in items.iter() {
            assert_eq!(
                archetype.items.contains(&tags, id),
                listed.contains(&item.as_str()),
                "{item} in the items of {name}"
            );
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "no archetype names a tag of direct items, so the test checks nothing"
    );
}

#[test]
fn the_local_light_tag_reaches_the_loaded_item_tags() {
    let set = test_registries();
    let items = set.entries::<Item>().unwrap();
    let tags = set.tags::<Item>().unwrap();
    let tag = tags
        .id(&TagKey::from_location(
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
    let set = test_registries();
    let bare: InstrumentValue = read_named(set, &instrument("item.goat_horn.sound.0")).unwrap();
    let written = serde_json::to_value(write_named(set, &bare)).unwrap();
    assert_eq!(written["sound_event"], "minecraft:item.goat_horn.sound.0");

    let upper = read_named::<InstrumentValue>(set, &instrument("minecraft:Item.goat_horn.sound.0"));
    assert!(upper.is_err());
}

#[test]
fn an_instrument_or_painting_the_game_refuses_fails_to_parse() {
    let set = test_registries();
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
    assert!(read_named::<InstrumentValue>(set, &instrument("")).is_ok());
    assert!(read_named::<InstrumentValue>(set, &instrument(r#", "volume": 1.0"#)).is_err());

    let painting =
        |width: u32| format!(r#"{{"asset_id": "minecraft:kebab", "width": {width}, "height": 1}}"#);
    assert!(read_named::<PaintingVariantValue>(set, &painting(16)).is_ok());
    assert!(read_named::<PaintingVariantValue>(set, &painting(17)).is_err());
}

#[test]
fn a_file_the_game_refuses_names_its_registry_entry_and_file() {
    let stone = r#"{"id":"minecraft:stone"}"#;
    let cases: [(&str, String, &[&str]); 9] = [
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
            "enchantment",
            enchantment_json(r##""#minecraft:nowhere""##, ""),
            &["minecraft:item", "minecraft:nowhere"],
        ),
        (
            "sulfur_cube_archetype",
            archetype_json("minecraft:no_such_attribute"),
            &["minecraft:attribute", "minecraft:no_such_attribute"],
        ),
    ];
    let refused = refused_each(
        &cases
            .iter()
            .map(|(registry, json, _)| (*registry, json.clone()))
            .collect::<Vec<_>>(),
    );
    for ((registry, json, parts), (name, text)) in cases.iter().zip(refused) {
        let entry = [
            format!("minecraft:{registry}"),
            format!("minecraft/{registry}/{name}.json"),
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
    let without_variants =
        VARIANT_REGISTRIES
            .iter()
            .fold(WithFiles::over_assets(), |source, registry| {
                let path = registry.trim_start_matches("minecraft:");
                source
                    .hiding(&format!("minecraft/{path}"))
                    .hiding(&format!("minecraft/tags/{path}"))
            });
    let errors = errors_of(without_variants.load());
    for registry in VARIANT_REGISTRIES {
        assert!(
            errors.iter().any(|error| matches!(
                error,
                Diagnostic::EmptyRegistry { registry: empty } if empty.as_str() == registry
            )),
            "{registry} is not refused as empty: {errors:?}"
        );
    }
}

#[test]
fn the_synced_wolf_variant_has_no_spawn_conditions() {
    let set = test_registries();
    let (_, wolves) = sent_by(set, false)
        .registries
        .into_iter()
        .find(|(registry, _)| registry.as_str() == "minecraft:wolf_variant")
        .expect("the wolf variants are synced");
    let synced = wolves
        .into_iter()
        .find(|(name, _)| name.as_str() == "minecraft:pale")
        .and_then(|(_, data)| data)
        .expect("the pale wolf is sent with its data");
    let synced = synced.extract_compound().expect("a variant is a compound");
    assert!(synced.get("assets").is_some());
    assert!(synced.get("spawn_conditions").is_none());

    let selectors = spawn_selectors::<WolfVariantFile>(set).unwrap();
    let pale = &selectors[&ResourceLocation::read("minecraft:pale").unwrap()];
    assert_eq!(
        serde_json::to_value(pale).unwrap(),
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

fn round_trip_named<T: Resolve>(json: &str) -> serde_json::Value
where
    T::Named: serde::de::DeserializeOwned + serde::Serialize,
{
    let value: T = read_named(test_registries(), json).unwrap_or_else(|e| panic!("{json}: {e}"));
    let text = serde_json::to_string(&write_named(test_registries(), &value)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn assert_samples_round_trip(
    type_registry: &str,
    round_trip: fn(&str) -> serde_json::Value,
    samples: &[(&str, &str)],
) {
    let sampled: BTreeSet<String> = samples.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert_eq!(
        sampled,
        registered_names(type_registry),
        "a sample for each type {type_registry} registers, and no other"
    );
    for (name, json) in samples {
        let written = round_trip(json);
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(written, read, "{name}");
        assert_eq!(written["type"], *name, "{json}");
    }
}

fn synced_named<T: Resolve>(json: &str) -> mcrs_minecraft_nbt::compound::NbtCompound
where
    T::Named: serde::de::DeserializeOwned + serde::Serialize,
{
    let value: T = read_named(test_registries(), json).unwrap_or_else(|e| panic!("{json}: {e}"));
    mcrs_minecraft_nbt::to_nbt_compound(&write_named(test_registries(), &value)).unwrap()
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
        "unresolved references #minecraft:nowhere",
        "minecraft:dialog/minecraft:odd",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text = dialog_list_in("known", "#minecraft:known");
    assert!(!text.contains("minecraft:dialog"), "{text}");
}

#[test]
fn a_dialog_shown_from_nested_text_must_exist() {
    let shown = |dialog: &str| {
        let notice = format!(
            r#"{{"type":"minecraft:notice","title":{{"text":"t","extra":[{{"text":"x",
                "click_event":{{"action":"show_dialog","dialog":"{dialog}"}}}}]}}}}"#
        );
        load_text(&[("minecraft/dialog/odd.json", &notice)])
    };
    let text = shown("minecraft:nowhere");
    for part in ["minecraft:dialog", "minecraft:nowhere", "minecraft:odd"] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
    let text = shown("minecraft:odd");
    assert!(!text.contains("minecraft:nowhere"), "{text}");
    assert!(!text.contains("Error"), "{text}");
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
    assert_samples_round_trip(
        "minecraft:dialog_type",
        round_trip_named::<Dialog>,
        DIALOG_SAMPLES,
    );
    assert_samples_round_trip(
        "minecraft:dialog_body_type",
        round_trip_named::<DialogBody>,
        BODY_SAMPLES,
    );
    assert_samples_round_trip(
        "minecraft:input_control_type",
        round_trip_named::<Input>,
        INPUT_SAMPLES,
    );
    assert_samples_round_trip(
        "minecraft:dialog_action_type",
        round_trip_named::<Action>,
        ACTION_SAMPLES,
    );

    let read = |json: &str| round_trip_named::<Dialog>(json);
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
#[expect(
    clippy::too_many_lines,
    reason = "one table of out-of-range dialog values, each against the field its error must name"
)]
fn a_dialog_value_outside_its_range_fails() {
    let dialog = |fields: &str| format!(r#"{{"title":"t",{fields}}}"#);
    let notice = |fields: &str| dialog(&format!(r#""type":"minecraft:notice",{fields}"#));
    let input = |control: &str| {
        notice(&format!(
            r#""inputs":[{{"key":"k","label":"L",{control}}}]"#
        ))
    };
    let action = |action: &str| notice(&format!(r#""action":{{"label":"x","action":{action}}}"#));

    let cases = [
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
            "Value must be positive: 0.0",
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
    ];
    let refused = refused_each(
        &cases
            .iter()
            .map(|(json, _)| ("dialog", json.clone()))
            .collect::<Vec<_>>(),
    );
    for ((json, part), (name, text)) in cases.iter().zip(refused) {
        assert!(
            text.contains(part),
            "{part} missing for {name} {json}:\n{text}"
        );
    }
}

#[test]
fn dialog_values_are_synced_with_the_tags_the_game_writes() {
    let dialog = synced_named::<Dialog>(
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
    assert_eq!(
        round_trip_named::<SulfurCubeArchetype>(&valid.to_string()),
        valid
    );

    let mut full = valid.clone();
    full["buoyant"] = serde_json::json!(true);
    full["explosion"] = serde_json::json!({"power": 3, "causes_fire": false, "fuse": 120});
    full["contact_damage"] = serde_json::json!({
        "damage_type": "minecraft:sulfur_cube_hot",
        "amount": {"type": "minecraft:uniform", "min_inclusive": 0.5, "max_exclusive": 1.5},
        "attribute_to_source": true,
    });
    assert_eq!(
        round_trip_named::<SulfurCubeArchetype>(&full.to_string()),
        full
    );

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
        let message = read_named::<SulfurCubeArchetype>(test_registries(), &json.to_string())
            .expect_err(pointer);
        assert!(message.contains(expected), "{pointer}: {message}");
    }
}

fn layered_asset_app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(
            &crate::common::pack_roots(),
            &crate::common::pack_layering(),
        ),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    app
}

fn built_in_files_read_through_the_layered_file_source(count: usize) {
    let app = layered_asset_app();
    let source = app
        .world()
        .resource::<AssetServer>()
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing")
        .reader();

    let paths = mcrs_minecraft_worldgen_builtin::template_paths("minecraft/structure");
    assert!(!paths.is_empty());
    for path in paths.into_iter().take(count) {
        bevy_tasks::block_on(read_whole(source, Path::new(&path)))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
}

#[test]
fn a_built_in_file_reads_through_the_layered_file_source() {
    built_in_files_read_through_the_layered_file_source(1);
}

fn timeline_file(clock: &str, markers: &str) -> String {
    format!(r#"{{"clock":"{clock}","period_ticks":24000,"time_markers":{markers}}}"#)
}

pub(crate) fn load_shipped_and(
    directory: &str,
    files: &[(&str, String)],
) -> Result<Registries, RegistryLoadError> {
    let files: Vec<(String, &str)> = files
        .iter()
        .map(|(name, json)| (format!("{directory}/{name}.json"), json.as_str()))
        .collect();
    WithFiles::test_pack(&files)
        .load()
        .map(|(registries, _)| registries)
}

#[test]
fn a_worldgen_file_that_does_not_parse_stops_the_load() {
    for registry in [
        "carver",
        "feature",
        "placed_feature",
        "processor_list",
        "structure",
        "structure_set",
    ] {
        let broken = format!("test/worldgen/{registry}/broken.json");
        let errors = errors_of(WithFiles::test_pack(&[(&broken, r#"{"bad":true}"#)]).load());
        let [
            Diagnostic::Entry {
                registry: refused,
                entry,
                file,
                message,
                ..
            },
        ] = errors.as_slice()
        else {
            panic!("{registry}: one entry error expected: {errors:?}");
        };
        assert_eq!(refused.as_str(), format!("minecraft:worldgen/{registry}"));
        assert_eq!(entry.as_str(), "test:broken");
        assert!(
            file.as_deref().is_some_and(|file| file.ends_with(&broken)),
            "{registry}: {file:?}"
        );
        assert!(!message.is_empty(), "{registry}: no reason");
    }
}

#[test]
fn a_time_marker_defined_twice_for_one_clock_fails_the_load() {
    let refused = load_shipped_and(
        "test/timeline",
        &[
            (
                "first",
                timeline_file("minecraft:overworld", r#"{"test:marker":1000}"#),
            ),
            (
                "second",
                timeline_file("minecraft:overworld", r#"{"test:marker":2000}"#),
            ),
        ],
    );
    let errors = errors_of(refused);
    let [
        Diagnostic::Entry {
            registry,
            entry,
            message,
            ..
        },
    ] = errors.as_slice()
    else {
        panic!("one entry error expected: {errors:?}");
    };
    assert_eq!(registry.as_str(), "minecraft:timeline");
    assert_eq!(
        entry.as_str(),
        "test:second",
        "the error names the second timeline"
    );
    assert!(message.contains("test:marker"), "{message}");
    assert!(message.contains("minecraft:overworld"), "{message}");
    assert!(message.contains("more than once"), "{message}");
}

#[test]
fn a_timeline_naming_an_unknown_clock_fails() {
    let refused = load_shipped_and(
        "test/timeline",
        &[("lost", timeline_file("minecraft:nowhere", "{}"))],
    )
    .expect_err("a clock the registry does not hold is refused")
    .to_string();

    for part in [
        "minecraft:world_clock",
        "unresolved references minecraft:nowhere",
        "minecraft:timeline/test:lost",
    ] {
        assert!(refused.contains(part), "{part} missing from:\n{refused}");
    }
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
    let set = test_registries();
    for loot in [
        "minecraft:loot_table",
        "minecraft:predicate",
        "minecraft:item_modifier",
    ] {
        assert!(
            set.names_of(&ResourceLocation::read(loot).unwrap())
                .is_some(),
            "{loot} has no names from the reloadable registries"
        );
    }

    let mut shipped = BTreeSet::new();
    json_names(&vanilla_dir().join("trial_spawner"), "", &mut shipped);
    let named: BTreeSet<String> = loaded_names(set, "minecraft:trial_spawner")
        .into_iter()
        .collect();
    assert!(!named.is_empty());
    assert_eq!(named, shipped);
}

pub fn worldgen_registries_are_parsed_by_the_loader_not_requested_as_assets(app: &App) {
    fn parsed<R: RegistryValue + 'static>(
        set: &Registries,
    ) -> (ResourceLocation, Vec<ResourceLocation>) {
        let registry = set.registry::<R>().unwrap_or_else(|| {
            panic!("{} is not parsed by the loader", std::any::type_name::<R>())
        });
        let names = registry.iter().map(|(_, name, _)| name.clone()).collect();
        (registry.key().into(), names)
    }

    let set: &Registries = app.world().resource::<CurrentRegistries>();
    let server = app.world().resource::<AssetServer>();

    let counts = [
        (parsed::<CarverConfig>(set), 5),
        (parsed::<Feature>(set), 262),
        (parsed::<PlacedFeature>(set), 300),
        (parsed::<StructureProcessorList>(set), 40),
        (parsed::<Structure>(set), 52),
        (parsed::<StructureSet>(set), 21),
    ];
    for ((registry, names), expected) in counts {
        assert_eq!(names.len(), expected, "{registry}");
        for name in names {
            let path = format!(
                "{}/{}/{}.json",
                name.namespace(),
                registry.path(),
                name.path()
            );
            assert!(
                server.get_path_ids(path.clone()).is_empty(),
                "{path} was requested as an asset"
            );
        }
    }

    let carvers = set.registry::<CarverConfig>().unwrap();
    assert!(
        carvers
            .by_name(&ResourceLocation::read("minecraft:beta_cave").unwrap())
            .is_some()
    );
}

fn overworld_dimension_type_with(
    changes: &[(&str, Option<&str>)],
) -> Result<DimensionTypeFile, String> {
    let mut file: serde_json::Value = serde_json::from_slice(
        &std::fs::read(vanilla_dir().join("dimension_type/overworld.json")).unwrap(),
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
    read_named(test_registries(), &file.to_string())
}

fn tag_in<R: Registered>(set: &Registries, name: &str) -> TagId<R> {
    set.tags::<R>()
        .unwrap_or_else(|| panic!("{} has loaded tags", std::any::type_name::<R>()))
        .id(&TagKey::from_location(
            ResourceLocation::read(name).unwrap(),
        ))
        .unwrap_or_else(|| panic!("{name} is a loaded tag"))
}

fn id_in<R: Registered>(set: &Registries, name: &str) -> Id<R> {
    set.id(&ResourceKey::from_location(
        ResourceLocation::read(name).unwrap(),
    ))
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn a_dimension_type_sends_has_fixed_time_only_when_set() {
    for (json, sent) in [("false", None), ("true", Some(NbtTag::Byte(1)))] {
        let read = overworld_dimension_type_with(&[("has_fixed_time", Some(json))]).unwrap();
        let nbt =
            mcrs_minecraft_nbt::to_nbt_compound(&read.name(test_registries()).unwrap()).unwrap();
        assert_eq!(nbt.get("has_fixed_time").cloned(), sent, "{json}");
    }
}

#[test]
fn a_dimension_type_reads_its_holder_fields_as_vanilla_does() {
    let set = test_registries();
    let stone = Block::Stone.id();
    let overworld_clock = id_in::<WorldClock>(set, "minecraft:overworld");

    let infiniburn: [(&str, IdSet<Block>); 3] = [
        (
            r##""#minecraft:infiniburn_overworld""##,
            IdSet::Tag(tag_in::<Block>(set, "minecraft:infiniburn_overworld")),
        ),
        (r#""minecraft:stone""#, IdSet::List(Box::new([stone]))),
        (r#"["minecraft:stone"]"#, IdSet::List(Box::new([stone]))),
    ];
    for (json, expected) in infiniburn {
        let read = overworld_dimension_type_with(&[("infiniburn", Some(json))])
            .unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(read.infiniburn, expected, "{json}");
    }

    let absent = overworld_dimension_type_with(&[("timelines", None), ("default_clock", None)])
        .expect("a dimension type without timelines or a clock reads");
    assert_eq!(absent.timelines, IdSet::default());
    assert_eq!(absent.default_clock, None);

    let shipped = overworld_dimension_type_with(&[]).expect("the shipped overworld reads");
    assert_eq!(
        shipped.timelines,
        IdSet::Tag(tag_in::<Timeline>(set, "minecraft:in_overworld"))
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

#[test]
fn every_preset_parses_in_registry_context() {
    let set = test_registries();
    let presets = set
        .registry::<WorldPreset>()
        .expect("the loader parses world presets");
    let preset = |name: &str| {
        presets
            .by_name(&ResourceLocation::read(name).unwrap())
            .and_then(|id| presets.get(id))
            .unwrap_or_else(|| panic!("no preset {name}"))
    };
    let overworld = |name: &str| &preset(name).dimensions["minecraft:overworld"].generator;

    let ChunkGenerator::Noise(normal) = overworld("minecraft:normal") else {
        panic!("the normal overworld is a noise generator");
    };
    let BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(list)) = &normal.biome_source else {
        panic!("the normal overworld uses a multi-noise preset");
    };
    assert_eq!(*list, id_in(set, "minecraft:overworld"));
    assert_eq!(normal.settings, id_in(set, "minecraft:overworld"));

    let ChunkGenerator::Noise(single) = overworld("minecraft:single_biome_surface") else {
        panic!("single_biome_surface is a noise generator");
    };
    assert_eq!(
        single.biome_source,
        BiomeSource::Fixed {
            biome: id_in(set, "minecraft:plains")
        }
    );

    assert!(matches!(
        overworld("minecraft:flat"),
        ChunkGenerator::Flat(_)
    ));
    assert_eq!(preset("minecraft:flat").dimensions.len(), 3);

    assert_eq!(
        overworld("minecraft:debug_all_block_states"),
        &ChunkGenerator::Debug
    );

    for (_, name, value) in presets.iter() {
        let written = serde_json::to_string(&write_named(set, value))
            .unwrap_or_else(|e| panic!("{name} does not encode: {e}"));
        let read: WorldPreset =
            read_named(set, &written).unwrap_or_else(|e| panic!("{name} does not read back: {e}"));
        assert_eq!(&read, value, "{name} changed across an encode and a parse");
    }
}

#[test]
fn a_preset_reads_its_dimensions_as_a_map_by_key() {
    let set = test_registries();
    let presets = set.registry::<WorldPreset>().unwrap();
    let preset = |name: &str| {
        presets
            .by_name(&ResourceLocation::read(name).unwrap())
            .and_then(|id| presets.get(id))
            .unwrap()
    };

    let beta = preset("minecraft:beta");
    assert_eq!(
        beta.dimensions
            .keys()
            .map(|key| key.as_str())
            .collect::<Vec<_>>(),
        ["minecraft:overworld"]
    );
    let beta_type: Id<DimensionType> = id_in(set, "minecraft:beta");
    assert_eq!(
        beta.dimensions["minecraft:overworld"].dimension_type,
        beta_type
    );

    let shipped: serde_json::Value = serde_json::from_slice(
        &std::fs::read(vanilla_dir().join("worldgen/world_preset/normal.json")).unwrap(),
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
        read_named(set, &text).unwrap_or_else(|e| panic!("{e}: {text}"))
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
        &std::fs::read(vanilla_dir().join("dimension_type/overworld.json")).unwrap(),
    )
    .unwrap();
    let text = format!(
        r#"{{"dimensions":{{"minecraft:overworld":{{"type":{inline},"generator":{{"type":"minecraft:debug"}}}}}}}}"#
    );
    let refused = refused_by_the_loader("worldgen/world_preset", "inline", &text);
    let line = refused
        .lines()
        .find(|line| line.contains("minecraft:inline"))
        .unwrap_or_else(|| {
            panic!("a dimension type written out in the preset is refused: {refused}")
        });
    assert!(
        line.contains("dimensions.minecraft:overworld.type"),
        "{line}"
    );
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
    let items = set.entries::<Item>().unwrap();
    let blocks = set.entries::<Block>().unwrap();
    let stone = ResourceLocation::read("minecraft:stone").unwrap();
    assert_eq!(items.by_name(&stone), Some(Item::Stone.id()));
    assert_eq!(blocks.by_name(&stone), Some(Block::Stone.id()));

    let mut differing = 0;
    for (item, name) in items.iter() {
        let Some(block) = blocks.by_name(name) else {
            continue;
        };
        assert_eq!(Item::find(name.as_str()).unwrap().id(), item, "{name}");
        assert_eq!(Block::find(name.as_str()).unwrap().id(), block, "{name}");
        differing += usize::from(item.number() != block.number());
    }
    assert!(differing > 0, "no shared name is numbered differently");
}

mod exhaustive {
    #[test]
    fn every_built_in_file_reads_through_the_layered_file_source() {
        super::built_in_files_read_through_the_layered_file_source(usize::MAX);
    }
}

#[test]
fn a_block_state_provider_names_only_providers_that_exist() {
    let rotated = |state: &str| format!(r#"{{"type":"minecraft:rotated","state":"{state}"}}"#);
    let path = "minecraft/worldgen/block_state_provider/odd.json";
    let missing = load_text(&[(path, &rotated("minecraft:no_such_provider"))]);
    assert!(missing.contains("minecraft:no_such_provider"), "{missing}");
    let present = load_text(&[(path, &rotated("minecraft:soil_beneath_tree"))]);
    assert!(present.is_empty(), "{present}");
}

#[test]
fn an_unknown_tag_in_a_spawn_condition_is_an_unresolved_reference_of_its_variant() {
    let wolf_assets = r#"{"wild":"minecraft:a","tame":"minecraft:b","angry":"minecraft:c"}"#;
    let json = format!(
        r##"{{"assets":{wolf_assets},"baby_assets":{wolf_assets},"spawn_conditions":[
            {{"condition":{{"type":"minecraft:biome","biomes":"#minecraft:no_such_tag"}},"priority":1}}]}}"##
    );
    let text = refused_by_the_loader("wolf_variant", "odd", &json);
    for part in [
        "minecraft:worldgen/biome",
        "#minecraft:no_such_tag",
        "minecraft:wolf_variant/minecraft:odd",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}
