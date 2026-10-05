use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mcrs_minecraft_core::registry_key::RegistryKey;
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
        mcrs_minecraft_registry::key::GameRule,
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
        mcrs_minecraft_world::chat_type::ChatType,
        mcrs_minecraft_world::enchantment_provider::EnchantmentProvider,
        mcrs_minecraft_world::sulfur_cube_archetype::SulfurCubeArchetype,
        mcrs_minecraft_world::test_types::TestInstance,
        mcrs_minecraft_world::villager_trade::TradeSet,
        mcrs_minecraft_world::villager_trade::VillagerTrade,
    ]
}

fn registries_of_the_reports() -> HashSet<String> {
    let static_registries: HashMap<String, IgnoredAny> = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let data_pack: DataPackReport = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/datapack.json"
    ))
    .unwrap();
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

const GRAPH_DOCUMENT: &str = "docs/registry-graph.md";

struct GraphRow {
    registries: Vec<String>,
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
