#![allow(dead_code)]

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
            .registry_of(mcrs_minecraft_keys::BLOCK)
            .expect("the registries report has blocks");
        Blocks(std::sync::Arc::new(
            load_block_definitions(&asset_server, &blocks)
                .expect("the corpus loads")
                .0,
        ))
    })
}

use mcrs_minecraft_worldgen_testing::{corpus_set, registry};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::compile::build_router;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};

pub fn router_blocks(blocks: &BlockDefinitions) -> RouterBlocks {
    RouterBlocks {
        default_block: blocks.default_state("minecraft:stone").0.into(),
        default_fluid: blocks.default_state("minecraft:water").0.into(),
        water: blocks.default_state("minecraft:water").0.into(),
        lava: blocks.default_state("minecraft:lava").0.into(),
    }
}

pub fn build_settings_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &ResourceLocation::minecraft(settings_name).unwrap(),
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
    use std::collections::BTreeMap;

    use mcrs_minecraft_worldgen_surface::compile::build_router_and_material;
    use mcrs_minecraft_worldgen_surface::{
        MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
    };

    use crate::block_state::try_resolve_state;

    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = registry("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        registry("material_condition");
    let biome_tags = biome_tags();
    let functions = registry("density_function");
    let noises = registry("noise");

    let mut seen = 0;
    for (id, settings) in registry::<NoiseGeneratorSettings>("noise_settings") {
        let name = id.path();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &|state| try_resolve_state(corpus(), state).map(|state| state.0.into()),
            biome_tags: &biome_tags,
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

use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_registry::{Registry, Tags};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;

pub fn text_ordered_table(
    registry: &str,
    names: impl IntoIterator<Item = ResourceLocation<std::sync::Arc<str>>>,
) -> std::sync::Arc<mcrs_minecraft_registry::NameTable> {
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    std::sync::Arc::new(
        mcrs_minecraft_registry::NameTable::new(
            ResourceLocation::read(registry).expect("a registry key"),
            names,
        )
        .expect("a table of distinct names"),
    )
}

/// Every block tag of the corpus: what the freeze hands the heightmap table and
/// the ore rule tests.
pub fn block_tags() -> &'static Tags<Block> {
    static TAGS: std::sync::OnceLock<Tags<Block>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| {
        corpus_set()
            .tags()
            .expect("the corpus set holds the block tags")
    })
}

/// Every biome of the corpus, numbered in text order.
pub fn corpus_biomes() -> &'static Registry<keys::Biome> {
    static REGISTRY: std::sync::OnceLock<Registry<keys::Biome>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        corpus_set()
            .registry()
            .expect("the corpus set holds the biome registry")
    })
}

/// Every corpus biome's climate, indexed like [`corpus_biomes`].
pub fn corpus_climate() -> &'static std::sync::Arc<[BiomeClimate]> {
    static CLIMATE: std::sync::OnceLock<std::sync::Arc<[BiomeClimate]>> =
        std::sync::OnceLock::new();
    CLIMATE.get_or_init(|| {
        let biomes = registry::<Biome>("biome");
        corpus_biomes()
            .ids()
            .map(|id| {
                let name = corpus_biomes()
                    .name(id)
                    .expect("an id of the registry has a name");
                let biome = &biomes[&ResourceLocation::read(name.as_str()).expect("a corpus id")];
                BiomeClimate {
                    base_temperature: biome.temperature,
                    frozen: biome.temperature_modifier == Some(TemperatureModifier::Frozen),
                    has_precipitation: biome.has_precipitation,
                }
            })
            .collect()
    })
}

pub fn biome_tags() -> Tags<keys::Biome> {
    corpus_set()
        .tags()
        .expect("the corpus set holds the biome tags")
}

pub fn structure_registry() -> &'static Registry<keys::Structure> {
    static REGISTRY: std::sync::OnceLock<Registry<keys::Structure>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        corpus_set()
            .registry()
            .expect("the corpus set holds the structure registry")
    })
}

pub fn structure_tags() -> Tags<keys::Structure> {
    corpus_set()
        .tags()
        .expect("the corpus set holds the structure tags")
}
