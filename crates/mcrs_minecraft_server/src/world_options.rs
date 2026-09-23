use crate::world::generate::routers::DimensionBiomeSources;
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_world::LoadedRegistryAssets;
use mcrs_minecraft_world::dimension::level_stem::DimensionDefinition;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::{ActiveWorldPreset, WorldPreset};
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Default world preset name used when MCRS_WORLD_PRESET is not set
const DEFAULT_WORLD_PRESET: &str = "normal";

pub(crate) fn start_loading_world_preset(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut registry_assets: ResMut<LoadedRegistryAssets>,
    mut loaded_preset: ResMut<LoadedWorldPreset>,
) {
    let preset_name = get_world_preset_name();
    let (namespace, path) = match preset_name.split_once(':') {
        Some((ns, p)) => (ns, p),
        None => ("minecraft", preset_name.as_str()),
    };
    let asset_path = format!("{namespace}/worldgen/world_preset/{path}.json");

    info!(
        preset = %preset_name,
        asset_path = %asset_path,
        "Starting to load world preset via Bevy asset system"
    );

    let handle: Handle<WorldPreset> = asset_server.load(asset_path);
    registry_assets.push(handle.clone().untyped());
    loaded_preset.preset_name = preset_name;
    commands.insert_resource(ActiveWorldPreset { handle });
}

pub(crate) fn process_loaded_world_preset(
    active: Option<Res<ActiveWorldPreset>>,
    presets: Res<Assets<WorldPreset>>,
    dim_defs: Res<Assets<DimensionDefinition>>,
    mut loaded_preset: ResMut<LoadedWorldPreset>,
    mut commands: Commands,
) {
    if !presets.is_changed() {
        return;
    }
    let Some(active) = active else {
        return;
    };
    let Some(preset) = presets.get(&active.handle) else {
        return;
    };

    let mut dimensions: Vec<(ResourceLocation, Handle<DimensionDefinition>)> = preset
        .dimensions
        .iter()
        .map(|(key, handle)| (key.location().clone(), handle.clone()))
        .collect();
    dimensions.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

    loaded_preset.dimensions = dimensions;
    loaded_preset.is_loaded = true;

    commands.insert_resource(dimension_biome_sources(&loaded_preset, &dim_defs));

    debug!(
        preset = %loaded_preset.preset_name,
        dimensions = loaded_preset.dimensions.len(),
        "World preset loaded"
    );
}

fn dimension_biome_sources(
    preset: &LoadedWorldPreset,
    dim_defs: &Assets<DimensionDefinition>,
) -> DimensionBiomeSources {
    let mut sources = DimensionBiomeSources::default();
    for (dimension, handle) in &preset.dimensions {
        let Some(definition) = dim_defs.get(handle) else {
            warn!(%dimension, "the dimension definition missing while the world preset loaded");
            continue;
        };
        let ChunkGenerator::Noise(generator) = &definition.generator else {
            continue;
        };
        sources
            .0
            .insert(dimension.clone(), Arc::new(generator.biome_source.clone()));
    }
    sources
}

/// Resource containing the loaded world preset with ordered dimensions.
/// The dimensions are sorted alphabetically by dimension key for deterministic ordering.
#[derive(Resource)]
pub struct LoadedWorldPreset {
    pub preset_name: String,
    pub dimensions: Vec<(ResourceLocation, Handle<DimensionDefinition>)>,
    pub is_loaded: bool,
}

impl Default for LoadedWorldPreset {
    fn default() -> Self {
        Self {
            preset_name: DEFAULT_WORLD_PRESET.to_string(),
            dimensions: Vec::new(),
            is_loaded: false,
        }
    }
}

/// The seed every dimension's noise router is compiled against. One writer in
/// the host; the routers carry it into the sub-apps.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldSeed(pub u64);

/// The seed `MCRS_WORLD_SEED` names. A save overrides it with the one stored in
/// its level data.
pub fn world_seed_from_env() -> WorldSeed {
    let Ok(raw) = env::var("MCRS_WORLD_SEED") else {
        return WorldSeed(0);
    };
    let raw = raw.trim();
    // A seed is a Java long, so it is written signed; the router hashes the
    // same bits either way.
    if let Ok(seed) = raw.parse::<i64>() {
        return WorldSeed(seed as u64);
    }
    match raw.parse::<u64>() {
        Ok(seed) => WorldSeed(seed),
        Err(error) => {
            error!(%error, raw, "MCRS_WORLD_SEED is not a number; generating with seed 0");
            WorldSeed(0)
        }
    }
}

/// Get the world preset name from the MCRS_WORLD_PRESET environment variable.
/// Returns the default 'normal' preset if not set or invalid.
/// Supports both short names ("normal") and namespaced identifiers ("minecraft:normal").
pub fn get_world_preset_name() -> String {
    match env::var("MCRS_WORLD_PRESET") {
        Ok(preset_name) => {
            let preset_name = preset_name.trim().to_lowercase();

            if preset_name.is_empty() {
                info!(
                    default_preset = DEFAULT_WORLD_PRESET,
                    "MCRS_WORLD_PRESET is empty, using default preset"
                );
                return DEFAULT_WORLD_PRESET.to_string();
            }

            info!(
                preset = %preset_name,
                "Loading world preset from MCRS_WORLD_PRESET"
            );

            preset_name
        }
        Err(_) => {
            info!(
                default_preset = DEFAULT_WORLD_PRESET,
                "MCRS_WORLD_PRESET not set, using default preset"
            );
            DEFAULT_WORLD_PRESET.to_string()
        }
    }
}
