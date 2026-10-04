use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Deserialize)]
struct DataPackReport {
    registries: HashMap<String, IgnoredAny>,
}

struct KeyType {
    name: &'static str,
    key: String,
}

fn key_type<T: RegistryKey>() -> KeyType {
    KeyType {
        name: std::any::type_name::<T>().rsplit("::").next().unwrap(),
        key: T::KEY.as_str().to_owned(),
    }
}

macro_rules! key_types {
    ($($ty:ty),* $(,)?) => {
        vec![$(key_type::<$ty>()),*]
    };
}

fn key_types() -> Vec<KeyType> {
    key_types![
        mcrs_minecraft_entity::EntityType,
        mcrs_minecraft_entity::Attribute,
        mcrs_minecraft_entity::VillagerType,
        mcrs_minecraft_entity::DamageType,
        mcrs_minecraft_entity::WolfVariant,
        mcrs_minecraft_entity::WolfSoundVariant,
        mcrs_minecraft_entity::PigVariant,
        mcrs_minecraft_entity::PigSoundVariant,
        mcrs_minecraft_entity::CowVariant,
        mcrs_minecraft_entity::CowSoundVariant,
        mcrs_minecraft_entity::ChickenVariant,
        mcrs_minecraft_entity::ChickenSoundVariant,
        mcrs_minecraft_entity::ZombieNautilusVariant,
        mcrs_minecraft_entity::FrogVariant,
        mcrs_minecraft_entity::CatVariant,
        mcrs_minecraft_entity::CatSoundVariant,
        mcrs_minecraft_entity::MobEffect,
        mcrs_minecraft_entity::GameEvent,
        mcrs_minecraft_entity::PointOfInterestType,
        mcrs_minecraft_item::key::Potion,
        mcrs_minecraft_item::key::Recipe,
        mcrs_minecraft_item::key::LootTable,
        mcrs_minecraft_item::key::MapDecorationType,
        mcrs_minecraft_item::key::Menu,
        mcrs_minecraft_item::key::ContextIntProvider,
        mcrs_minecraft_item::key::ContextFloatProvider,
        mcrs_minecraft_item::Item,
        mcrs_minecraft_item::enchantment::data::EnchantmentData,
        mcrs_minecraft_item::component::sound::SoundEvent,
        mcrs_minecraft_item::component::banner::BannerPattern,
        mcrs_minecraft_item::component::instrument::InstrumentValue,
        mcrs_minecraft_item::component::instrument::JukeboxSong,
        mcrs_minecraft_item::component::painting::PaintingVariantValue,
        mcrs_minecraft_item::component::trim::TrimMaterial,
        mcrs_minecraft_item::component::trim::TrimPattern,
        mcrs_minecraft_registry::key::BlockTransformer,
        mcrs_minecraft_registry::key::DecoratedPotPattern,
        mcrs_minecraft_registry::key::BlockEntityType,
        mcrs_minecraft_registry::key::Dimension,
        mcrs_minecraft_environment::timeline::Timeline,
        mcrs_minecraft_registry::key::Block,
        mcrs_minecraft_registry::key::Fluid,
        mcrs_minecraft_registry::key::Dialog,
        mcrs_minecraft_registry::key::Biome,
        mcrs_minecraft_registry::key::Structure,
        mcrs_minecraft_registry::key::ParticleType,
        mcrs_minecraft_registry::key::Carver,
        mcrs_minecraft_registry::key::EnvironmentAttribute,
        mcrs_minecraft_registry::key::Activity,
        mcrs_minecraft_registry::key::DimensionType,
        mcrs_minecraft_registry::key::TemplatePool,
        mcrs_minecraft_registry::key::MaterialRule,
        mcrs_minecraft_registry::key::BlockStateProvider,
        mcrs_minecraft_registry::key::MultiNoiseBiomeSourceParameterList,
        mcrs_minecraft_registry::key::VillagerProfession,
        mcrs_minecraft_registry::key::ContextKeySet,
        mcrs_minecraft_registry::key::TestFunction,
        mcrs_minecraft_registry::key::TestInstanceType,
        mcrs_minecraft_registry::key::TestEnvironmentDefinitionType,
        mcrs_minecraft_environment::world_clock::WorldClock,
        mcrs_minecraft_worldgen_feature::proto::PlacedFeature,
        mcrs_minecraft_worldgen_feature::proto::Feature,
        mcrs_minecraft_worldgen_feature::proto::StructureProcessorList,
        mcrs_minecraft_worldgen_structure::StructureSet,
        mcrs_minecraft_worldgen_noise::proto::NoiseParam,
        mcrs_minecraft_worldgen_density::proto::ProtoDensityFunction,
        mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings,
        mcrs_minecraft_worldgen_surface::proto::MaterialCondition,
        mcrs_minecraft_anvil::ChunkStatus,
        mcrs_minecraft_protocol::recipe::RecipeBookCategory,
        mcrs_minecraft_item::ItemComponentKind,
        mcrs_minecraft_world::test_types::TestEnvironment,
    ]
}

fn registries_of_the_reports() -> HashSet<String> {
    let static_registries: HashMap<String, IgnoredAny> =
        serde_json::from_str(include_str!("../../../assets/mcrs/reports/registries.json")).unwrap();
    let data_pack: DataPackReport =
        serde_json::from_str(include_str!("../../../assets/mcrs/reports/datapack.json")).unwrap();
    static_registries
        .into_keys()
        .chain(data_pack.registries.into_keys())
        .collect()
}

#[test]
fn every_key_names_a_registry_of_the_reports() {
    let registries = registries_of_the_reports();
    assert!(!registries.is_empty(), "the reports name no registry");
    let keys = key_types();
    assert!(!keys.is_empty(), "the list holds no key type");
    let unknown: Vec<String> = keys
        .iter()
        .filter(|k| !registries.contains(&k.key))
        .map(|k| format!("{} names {}", k.name, k.key))
        .collect();
    assert!(
        unknown.is_empty(),
        "keys that name no registry of either report:\n{}",
        unknown.join("\n")
    );
}

#[test]
fn no_two_keys_name_one_registry() {
    let mut seen: HashMap<String, &'static str> = HashMap::new();
    let mut clashes = Vec::new();
    for k in key_types() {
        if let Some(first) = seen.insert(k.key.clone(), k.name) {
            clashes.push(format!("{first} and {} both name {}", k.name, k.key));
        }
    }
    assert!(
        clashes.is_empty(),
        "two key types name one registry:\n{}",
        clashes.join("\n")
    );
}

struct Found {
    name: String,
    file: String,
    line: usize,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn crate_sources() -> Vec<PathBuf> {
    let crates = workspace_root().join("crates");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap() {
        let src = entry.unwrap().path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    files.sort();
    files
}

fn type_name_after(line: &str, marker: &str) -> Option<String> {
    let rest = &line[line.find(marker)? + marker.len()..];
    let path: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | ':'))
        .collect();
    path.rsplit("::")
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn scan(is_impl_line: impl Fn(&str) -> Option<String>) -> Vec<Found> {
    let root = workspace_root();
    let mut found = Vec::new();
    for path in crate_sources() {
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if let Some(name) = is_impl_line(line) {
                found.push(Found {
                    name,
                    file: path.strip_prefix(&root).unwrap().display().to_string(),
                    line: index + 1,
                });
            }
        }
    }
    found
}

fn key_impls() -> Vec<Found> {
    scan(|line| {
        line.starts_with("impl RegistryKey for ")
            .then(|| type_name_after(line, "impl RegistryKey for "))
            .flatten()
    })
}

fn tag_marker_impls() -> Vec<Found> {
    scan(|line| {
        (line.starts_with("impl") && line.contains("TaggedRegistry for "))
            .then(|| type_name_after(line, "TaggedRegistry for "))
            .flatten()
    })
}

struct TagMarker {
    name: &'static str,
    registry_path: &'static str,
    key: String,
}

fn tag_marker<T: RegistryKey + TaggedRegistry>() -> TagMarker {
    TagMarker {
        name: std::any::type_name::<T>().rsplit("::").next().unwrap(),
        registry_path: T::REGISTRY_PATH,
        key: T::KEY.as_str().to_owned(),
    }
}

macro_rules! tag_markers {
    ($($ty:ty),* $(,)?) => {
        vec![$(tag_marker::<$ty>()),*]
    };
}

fn tag_markers() -> Vec<TagMarker> {
    tag_markers![
        mcrs_minecraft_item::Item,
        mcrs_minecraft_item::enchantment::data::EnchantmentData,
        mcrs_minecraft_registry::key::Block,
        mcrs_minecraft_registry::key::Fluid,
        mcrs_minecraft_entity::EntityType,
        mcrs_minecraft_registry::key::Dialog,
        mcrs_minecraft_environment::timeline::Timeline,
        mcrs_minecraft_registry::key::Biome,
        mcrs_minecraft_registry::key::Structure,
    ]
}

fn registry_path(key: &str) -> &str {
    key.split_once(':').unwrap().1
}

#[test]
fn the_list_holds_every_key_type_of_the_sources() {
    let impls = key_impls();
    assert!(!impls.is_empty(), "the scan found no key impl");

    let mut places: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for found in &impls {
        places
            .entry(&found.name)
            .or_default()
            .push(format!("{}:{}", found.file, found.line));
    }

    let listed: BTreeSet<&str> = key_types().iter().map(|k| k.name).collect();
    let mut offences = Vec::new();
    for (name, at) in &places {
        if !listed.contains(name) {
            offences.push(format!(
                "{name} implements the trait at {} but is not listed",
                at.join(", ")
            ));
        }
        if at.len() > 1 {
            offences.push(format!(
                "{name} implements the trait in {} places: {}",
                at.len(),
                at.join(", ")
            ));
        }
    }
    for name in &listed {
        if !places.contains_key(name) {
            offences.push(format!("{name} is listed but no impl of it starts a line"));
        }
    }
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn a_tag_markers_key_has_its_tag_path() {
    let markers = tag_markers();
    assert!(!markers.is_empty(), "the list holds no tag marker");
    let offences: Vec<String> = markers
        .iter()
        .filter(|m| registry_path(&m.key) != m.registry_path || !m.key.starts_with("minecraft:"))
        .map(|m| {
            format!(
                "{}: key {} against tag path {}",
                m.name, m.key, m.registry_path
            )
        })
        .collect();
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn every_tag_marker_is_a_key() {
    let impls = tag_marker_impls();
    assert!(!impls.is_empty(), "the scan found no tag marker impl");
    let known: HashSet<&str> = tag_markers().iter().map(|m| m.name).collect();
    let offences: Vec<String> = impls
        .iter()
        .filter(|found| !known.contains(found.name.as_str()))
        .map(|found| {
            format!(
                "{}:{}: {} is a tag marker with no key in the list",
                found.file, found.line, found.name
            )
        })
        .collect();
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn every_marker_registry_has_a_key() {
    let holder = workspace_root().join("crates/mcrs_minecraft_registry/src/holder.rs");
    let text = std::fs::read_to_string(&holder).unwrap();
    let rest = text
        .split_once("registries! {")
        .expect("holder.rs has no registries! invocation")
        .1;
    let body = if rest.starts_with('}') {
        ""
    } else {
        rest.split_once("\n}")
            .expect("the registries! invocation is not closed")
            .0
    };
    let paths: Vec<&str> = body
        .lines()
        .filter_map(|line| {
            line.split_once('=')?
                .1
                .trim()
                .strip_prefix('"')?
                .split_once('"')
        })
        .map(|(path, _)| path)
        .collect();
    assert!(
        !paths.is_empty() || body.trim().is_empty(),
        "no registry path read from holder.rs"
    );

    let keys = key_types();
    let missing: Vec<&str> = paths
        .into_iter()
        .filter(|path| !keys.iter().any(|k| registry_path(&k.key) == *path))
        .collect();
    assert!(
        missing.is_empty(),
        "registries named by a marker with no key: {missing:?}"
    );
}

#[test]
fn the_three_registries_without_a_referrer_type_have_keys() {
    let keys = key_types();
    for path in ["menu", "game_event", "point_of_interest_type"] {
        assert!(
            keys.iter().any(|k| registry_path(&k.key) == path),
            "no key names {path}"
        );
    }
}

const GRAPH_DOCUMENT: &str = "docs/registry-graph.md";

struct GraphRow {
    registries: Vec<String>,
    references: Vec<String>,
    referred_to: bool,
}

fn backticked(cell: &str) -> Vec<String> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

fn graph_rows_of(text: &str, document: &str) -> Vec<GraphRow> {
    let section = text
        .split_once("\n## Registries\n")
        .unwrap_or_else(|| panic!("{document} has no Registries section"))
        .1;
    let section = section
        .split_once("\n## ")
        .map_or(section, |(body, _)| body);
    let rows: Vec<GraphRow> = section
        .lines()
        .filter(|line| line.starts_with('|') && line[1..].trim_start().starts_with('`'))
        .map(|line| {
            let cells: Vec<&str> = line
                .trim()
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            assert_eq!(
                cells.len(),
                6,
                "{document}: a registries row has {} cells instead of 6: {line}",
                cells.len()
            );
            GraphRow {
                registries: backticked(cells[0]),
                references: backticked(cells[2]),
                referred_to: cells[3] != "nothing",
            }
        })
        .collect();
    assert!(!rows.is_empty(), "{document} has no registries row");
    rows
}

fn graph_rows() -> Vec<GraphRow> {
    let path = workspace_root().join(GRAPH_DOCUMENT);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    graph_rows_of(&text, GRAPH_DOCUMENT)
}

#[test]
fn every_key_is_a_row_of_the_graph() {
    let rows = graph_rows();
    let listed: HashSet<&str> = rows
        .iter()
        .flat_map(|row| row.registries.iter().map(String::as_str))
        .collect();
    let offences: Vec<String> = key_types()
        .iter()
        .filter(|k| !listed.contains(registry_path(&k.key)))
        .map(|k| {
            format!(
                "{} names {}, which has no row in {GRAPH_DOCUMENT}",
                k.name, k.key
            )
        })
        .collect();
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn the_graph_is_closed_under_its_references() {
    let rows = graph_rows();
    let referred: HashSet<&str> = rows
        .iter()
        .filter(|row| row.referred_to)
        .flat_map(|row| row.registries.iter().map(String::as_str))
        .collect();
    let mut offences = Vec::new();
    for row in &rows {
        for reference in &row.references {
            if !referred.contains(reference.as_str()) {
                offences.push(format!(
                    "{} refers to {reference}, which has no row naming a referrer",
                    row.registries.join(", ")
                ));
            }
        }
    }
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn a_document_without_the_registries_table_is_not_read_as_empty() {
    let section = std::panic::catch_unwind(|| graph_rows_of("# Title\n\nno table\n", "fixture"));
    assert!(section.is_err(), "a document with no section was read");
    let rows = std::panic::catch_unwind(|| {
        graph_rows_of(
            "## Registries\n\n| a | b |\n|---|---|\n\n## Next\n",
            "fixture",
        )
    });
    assert!(rows.is_err(), "a section with no rows was read");
}

#[test]
fn every_registry_the_graph_lists_as_referred_to_has_a_key() {
    let keyed: HashSet<String> = key_types().into_iter().map(|k| k.key).collect();
    let mut missing = BTreeSet::new();
    for row in graph_rows().iter().filter(|row| row.referred_to) {
        for path in &row.registries {
            if !keyed.contains(&format!("minecraft:{path}")) {
                missing.insert(path.clone());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "registries the graph lists as referred to, with no key:\n{}",
        missing.into_iter().collect::<Vec<_>>().join("\n")
    );
}
