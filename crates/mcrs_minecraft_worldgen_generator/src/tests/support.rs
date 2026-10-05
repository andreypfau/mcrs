#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, load_block_definitions};
use mcrs_minecraft_registry::static_report::shipped_report;

/// The corpus, loaded once per process. Worldgen resolves every block it
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
        let blocks = shipped_report()
            .registry::<Block>()
            .expect("the registries report has blocks");
        Blocks(std::sync::Arc::new(
            load_block_definitions(&asset_server, &blocks)
                .expect("the corpus loads")
                .0,
        ))
    })
}

use mcrs_minecraft_worldgen_testing::registry;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::compile::build_router;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};

pub fn router_blocks(blocks: &BlockDefinitions) -> RouterBlocks {
    RouterBlocks {
        default_block: blocks.default_state("minecraft:stone").into(),
        default_fluid: blocks.default_state("minecraft:water").into(),
        water: blocks.default_state("minecraft:water").into(),
        lava: blocks.default_state("minecraft:lava").into(),
    }
}

pub fn build_settings_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &ResourceLocation::minecraft(settings_name),
    );
    build_router(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
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
    use std::collections::{BTreeMap, HashMap};

    use mcrs_minecraft_worldgen_surface::compile::build_router_and_material;
    use mcrs_minecraft_worldgen_surface::{
        MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
    };

    use crate::block_state::try_resolve_state;

    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = registry("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        registry("material_condition");
    let biome_ids: HashMap<String, u16> = registry::<serde::de::IgnoredAny>("biome")
        .into_keys()
        .enumerate()
        .map(|(id, name)| (name.as_str().to_owned(), u16::try_from(id).unwrap()))
        .collect();
    let functions = registry("density_function");
    let noises = registry("noise");

    let mut seen = 0;
    for (id, settings) in registry::<NoiseGeneratorSettings>("noise_settings") {
        let name = id.path();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &|state| try_resolve_state(corpus(), state).map(Into::into),
            biome: &|id| biome_ids.get(id.as_str()).copied(),
        };
        build_router_and_material(
            &settings,
            &functions,
            &noises,
            42,
            router_blocks(corpus()),
            &inputs,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        seen += 1;
    }
    assert!(seen >= 8, "only {seen} noise settings were checked");
}

use std::collections::HashSet;

use mcrs_minecraft_assets::tag::TagLoader;
use mcrs_minecraft_assets::tag::file::SerializedTagFile;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_block::definition::Fluids;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_registry::TagSource;
use mcrs_minecraft_registry::key;
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::key::Fluid;
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;

pub fn text_ordered_table(
    registry: &str,
    names: impl IntoIterator<Item = ResourceLocation<std::sync::Arc<str>>>,
) -> std::sync::Arc<mcrs_minecraft_registry::NameTable> {
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    std::sync::Arc::new(
        mcrs_minecraft_registry::NameTable::new(
            ResourceLocation::parse(registry).expect("a registry key"),
            names,
            [],
        )
        .expect("a table of distinct names"),
    )
}

fn tag_dir(registry: &str) -> PathBuf {
    mcrs_minecraft_worldgen_testing::assets_dir()
        .join("minecraft/tags")
        .join(registry)
}

/// One block tag of the corpus, expanded off the files themselves.
pub fn tag_members(name: &str) -> HashSet<u16> {
    let mut members = HashSet::new();
    collect_tag_members(Block::KEY.path(), blocks(), name, &mut members);
    members
}

fn collect_tag_members<S: TagSource<Id = u16>>(
    registry: &str,
    source: &S,
    name: &str,
    into: &mut HashSet<u16>,
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
fn every_tag<T: RegistryKey, S: TagSource<Id = u16>>(source: &S) -> DynTagRegistry<T> {
    let dir = tag_dir(T::KEY.path());
    let mut loader = TagLoader::<T, u16>::default();
    for path in mcrs_minecraft_worldgen_testing::json_files(&dir) {
        let relative = path.strip_prefix(&dir).unwrap().with_extension("");
        let name = format!(
            "minecraft:{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        let mut members = HashSet::new();
        collect_tag_members(T::KEY.path(), source, &name, &mut members);
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
pub fn biome_index() -> &'static DynRegistryIndex<key::Biome> {
    static INDEX: std::sync::OnceLock<DynRegistryIndex<key::Biome>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        DynRegistryIndex::from_table(&text_ordered_table(
            "minecraft:worldgen/biome",
            registry::<serde::de::IgnoredAny>("biome").into_keys(),
        ))
    })
}

/// Every corpus biome's climate, indexed like [`biome_index`].
pub fn corpus_climate() -> &'static std::sync::Arc<[BiomeClimate]> {
    static CLIMATE: std::sync::OnceLock<std::sync::Arc<[BiomeClimate]>> =
        std::sync::OnceLock::new();
    CLIMATE.get_or_init(|| {
        let biomes = registry::<Biome>("biome");
        (0..=u16::MAX)
            .take(usize::try_from(biome_index().len()).unwrap())
            .map(|id| {
                let name = biome_index()
                    .location(id)
                    .expect("an index below the length");
                let biome = &biomes[&ResourceLocation::parse(name.as_str()).expect("a corpus id")];
                BiomeClimate {
                    base_temperature: biome.temperature,
                    frozen: biome.temperature_modifier == Some(TemperatureModifier::Frozen),
                    has_precipitation: biome.has_precipitation,
                }
            })
            .collect()
    })
}

pub fn biome_tags() -> &'static DynTagRegistry<key::Biome> {
    static TAGS: std::sync::OnceLock<DynTagRegistry<key::Biome>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| every_tag(biome_index()))
}

pub fn structure_index() -> &'static DynRegistryIndex<key::Structure> {
    static INDEX: std::sync::OnceLock<DynRegistryIndex<key::Structure>> =
        std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        DynRegistryIndex::from_table(&text_ordered_table(
            "minecraft:worldgen/structure",
            registry::<serde::de::IgnoredAny>("structure").into_keys(),
        ))
    })
}

pub fn structure_tags() -> &'static DynTagRegistry<key::Structure> {
    static TAGS: std::sync::OnceLock<DynTagRegistry<key::Structure>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| every_tag(structure_index()))
}
