#![allow(dead_code)]

use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, load_block_definitions};

/// The corpus, loaded once per test binary. Worldgen resolves every block it
/// places against it, so a stub would fail at the first lookup.
pub fn corpus() -> &'static BlockDefinitions {
    &blocks().0
}

/// The corpus as the generation systems take it, sharing the one load above.
pub fn blocks() -> &'static Blocks {
    static CORPUS: OnceLock<Blocks> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        Blocks(std::sync::Arc::new(
            load_block_definitions(&asset_server)
                .expect("the corpus loads")
                .0,
        ))
    })
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::proto::{DensityFunctionHolder, NoiseParam};
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};

pub fn router_blocks(blocks: &BlockDefinitions) -> RouterBlocks {
    RouterBlocks {
        default_block: blocks.default_state("minecraft:stone").into(),
        default_fluid: blocks.default_state("minecraft:water").into(),
        water: blocks.default_state("minecraft:water").into(),
        lava: blocks.default_state("minecraft:lava").into(),
    }
}

pub fn assets_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
}

pub fn load_json_dir<T: serde::de::DeserializeOwned>(name: &str) -> BTreeMap<ResourceLocation, T> {
    mcrs_minecraft_worldgen::corpus::registry(name)
}

pub fn density_function_registry() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    load_json_dir("density_function")
}

pub fn noise_registry() -> BTreeMap<ResourceLocation, NoiseParam> {
    load_json_dir("noise")
}

pub fn build_settings_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let path = assets_root().join(format!("noise_settings/{settings_name}.json"));
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{settings_name}: {e}"));
    build_router(
        &settings,
        &density_function_registry(),
        &noise_registry(),
        seed,
        router_blocks(corpus()),
        None,
    )
    .unwrap_or_else(|e| panic!("{settings_name}: {e}"))
}

pub fn build_beta_router() -> NoiseRouter {
    build_settings_router("beta", 12345)
}

/// A block state or biome name the registries do not resolve fails the whole
/// compile, and the router the app then never inserts is what gates column
/// generation, so a corpus rename would otherwise cost every chunk silently.
#[test]
fn every_shipped_noise_settings_compiles_its_material_rules() {
    use std::collections::HashMap;

    use mcrs_minecraft_worldgen::material::{
        MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
    };

    use crate::world::chunk::try_resolve_state;

    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = load_json_dir("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        load_json_dir("material_condition");
    let biome_ids: HashMap<String, u32> = load_json_dir::<serde::de::IgnoredAny>("biome")
        .into_keys()
        .enumerate()
        .map(|(id, name)| (name.as_str().to_owned(), id as u32))
        .collect();
    let functions = density_function_registry();
    let noises = noise_registry();

    let dir = assets_root().join("noise_settings");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let json = std::fs::read_to_string(&path).unwrap();
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &|state| try_resolve_state(corpus(), state).map(Into::into),
            biome: &|id| biome_ids.get(id.as_str()).copied(),
        };
        let router = build_router(
            &settings,
            &functions,
            &noises,
            42,
            router_blocks(corpus()),
            Some(&inputs),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            router.material().is_some(),
            "{name} has no material program"
        );
        seen += 1;
    }
    assert!(seen >= 8, "only {seen} noise settings were checked");
}

use std::collections::HashSet;

use mcrs_minecraft_assets::tag::TagLoader;
use mcrs_minecraft_assets::tag::file::SerializedTagFile;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_assets::tag::registry::TagSource;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Fluids;
use mcrs_minecraft_block::{Block, Fluid};
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_registry::DynRegistryIndex;

fn tag_dir(registry: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../assets/minecraft/tags/{registry}"))
}

/// One block tag of the corpus, expanded off the files themselves.
pub fn tag_members(name: &str) -> HashSet<u32> {
    let mut members = HashSet::new();
    collect_tag_members(Block::REGISTRY_PATH, blocks(), name, &mut members);
    members
}

fn collect_tag_members<S: TagSource<Id = u32>>(
    registry: &str,
    source: &S,
    name: &str,
    into: &mut HashSet<u32>,
) {
    let path = tag_dir(registry).join(format!("{}.json", name.trim_start_matches("minecraft:")));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let file: SerializedTagFile = serde_json::from_str(&text).expect("a tag file");
    for entry in file.values {
        if entry.id.is_tag {
            collect_tag_members(registry, source, entry.id.loc.as_str(), into);
        } else if let Some(index) = source.id_of(entry.id.loc.as_str()) {
            into.insert(index);
        }
    }
}

/// Every tag file of one registry, subfolders included, expanded off the
/// files themselves.
fn every_tag<T: TaggedRegistry, S: TagSource<Id = u32>>(source: &S) -> DynTagRegistry<T> {
    let dir = tag_dir(T::REGISTRY_PATH);
    let mut loader = TagLoader::<T, u32>::new(&[]);
    for path in mcrs_minecraft_worldgen::corpus::json_files(&dir) {
        let relative = path.strip_prefix(&dir).unwrap().with_extension("");
        let name = format!(
            "minecraft:{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        let mut members = HashSet::new();
        collect_tag_members(T::REGISTRY_PATH, source, &name, &mut members);
        loader.insert(
            ResourceLocation::parse(&name).expect("a tag id").to_arc(),
            members,
        );
    }
    loader.freeze(source)
}

/// Every block tag of the corpus: what the freeze hands the heightmap table and
/// the ore rule tests.
pub fn block_tags() -> &'static DynTagRegistry<Block> {
    static TAGS: std::sync::OnceLock<DynTagRegistry<Block>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| every_tag(blocks()))
}

pub fn fluid_tags() -> &'static DynTagRegistry<Fluid> {
    static TAGS: std::sync::OnceLock<DynTagRegistry<Fluid>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| every_tag(&Fluids(blocks().0.clone())))
}

/// Every biome id of the corpus, numbered the way the snapshot numbers them.
pub fn biome_index() -> &'static DynRegistryIndex<Biome> {
    static INDEX: std::sync::OnceLock<DynRegistryIndex<Biome>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        DynRegistryIndex::build(load_json_dir::<serde::de::IgnoredAny>("biome").into_keys())
    })
}

pub fn biome_tags() -> &'static DynTagRegistry<Biome> {
    static TAGS: std::sync::OnceLock<DynTagRegistry<Biome>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| every_tag(biome_index()))
}
