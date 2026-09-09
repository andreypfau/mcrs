#![allow(dead_code)]

use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_world::block::definition::{BlockDefinitions, Blocks, load_block_definitions};

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
use std::path::{Path, PathBuf};

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

fn walk_json(base: &Path, dir: &Path, out: &mut Vec<(ResourceLocation, String)>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk_json(base, &path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let rel = path.strip_prefix(base).unwrap().with_extension("");
            let key = format!("minecraft:{}", rel.to_string_lossy().replace('\\', "/"));
            let ident = key
                .parse::<ResourceLocation>()
                .unwrap_or_else(|e| panic!("{key}: {e:?}"));
            let json = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.push((ident, json));
        }
    }
}

pub fn load_json_dir<T: serde::de::DeserializeOwned>(name: &str) -> BTreeMap<ResourceLocation, T> {
    let dir = assets_root().join(name);
    let mut files = Vec::new();
    walk_json(&dir, &dir, &mut files);
    files
        .into_iter()
        .map(|(id, json)| {
            let value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{id}: {e}"));
            (id, value)
        })
        .collect()
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
