use crate::WorldSave;
use crate::world::generate::routers::DimensionBiomeSources;
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};
use mcrs_minecraft_world::LoadedRegistryAssets;
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::registries::refuse;
use mcrs_minecraft_world::save::read_world_gen_settings;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_worldgen::bevy::NoiseGeneratorSettingsAsset;
use mcrs_minecraft_worldgen::tables::asset_path;
use std::env;
use std::ops::Deref;
use std::sync::Arc;
use tracing::{error, info};

/// Default world preset name used when MCRS_WORLD_PRESET is not set
const DEFAULT_WORLD_PRESET: &str = "normal";

pub fn configured_preset(
    name: &str,
    set: &RegistrySet,
    report: &mut LoadReport,
) -> Option<Id<keys::WorldPreset>> {
    let registry = report.registry(set, keys::WORLD_PRESET)?;
    report.require_by_name(&registry, name)
}

pub(crate) fn bake_dimensions(
    mut commands: Commands,
    set: Res<RegistrySet>,
    save: Option<Res<WorldSave>>,
    mut seed: ResMut<WorldSeed>,
) {
    let name = get_world_preset_name();
    let mut report = LoadReport::new();
    let Some(preset) = configured_preset(&name, &set, &mut report) else {
        refuse(&report)
    };

    let mut base = Dimensions::new();
    if let Some(save) = save {
        let settings = read_world_gen_settings(&save.0, &set).unwrap_or_else(|err| panic!("{err}"));
        seed.0 = settings.seed as u64;
        base = settings.dimensions;
    }
    if base.is_empty() {
        base = set
            .entries::<keys::WorldPreset, WorldPreset>()
            .expect("the data pack loader parses minecraft:worldgen/world_preset")[preset]
            .dimensions
            .clone();
    }
    let Some(list) = bake(&base, &set, &mut report) else {
        refuse(&report)
    };

    let mut sources = DimensionBiomeSources::default();
    for (dimension, entry) in &list {
        if let ChunkGenerator::Noise(generator) = &entry.generator {
            sources.0.insert(
                dimension.location().clone(),
                Arc::new(generator.biome_source.clone()),
            );
        }
    }
    info!(preset = %name, dimensions = list.len(), "dimension list");
    commands.insert_resource(sources);
    commands.insert_resource(DimensionList::new(list));
}

/// Only the noise settings the baked dimensions name are loaded: the rest of the
/// registry is names, and a dimension nobody spawns costs no asset.
pub(crate) fn request_dimension_noise_settings(
    dimensions: Res<DimensionList>,
    set: Res<RegistrySet>,
    asset_server: Res<AssetServer>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    let settings = set
        .registry::<keys::NoiseSettings>()
        .expect("the data pack declares minecraft:worldgen/noise_settings");
    for (_, entry) in dimensions.iter() {
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let Some(name) = settings.name(generator.settings) else {
            error!(id = ?generator.settings, "the dimension names noise settings the registry does not number");
            continue;
        };
        let handle = asset_server.load::<NoiseGeneratorSettingsAsset>(asset_path(
            &keys::NOISE_SETTINGS.location(),
            name,
        ));
        loaded.push(handle.untyped());
    }
}

/// Every dimension the world spawns, in spawn order. Written once at `Startup`.
#[derive(Resource, Clone)]
pub struct DimensionList {
    entries: Arc<[(ResourceKey<keys::Dimension>, DimensionEntry)]>,
    keys: Arc<[ResourceKey<keys::Dimension>]>,
}

impl DimensionList {
    pub fn new(entries: Vec<(ResourceKey<keys::Dimension>, DimensionEntry)>) -> Self {
        let keys = entries.iter().map(|(key, _)| key.clone()).collect();
        DimensionList {
            entries: entries.into(),
            keys,
        }
    }

    pub fn keys(&self) -> &Arc<[ResourceKey<keys::Dimension>]> {
        &self.keys
    }
}

impl Deref for DimensionList {
    type Target = [(ResourceKey<keys::Dimension>, DimensionEntry)];

    fn deref(&self) -> &Self::Target {
        &self.entries
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
    env::var("MCRS_WORLD_PRESET")
        .ok()
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_WORLD_PRESET.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_world::registries::test_registries;

    #[test]
    fn an_unknown_world_preset_is_refused_with_the_report() {
        let set = test_registries();
        let mut report = LoadReport::new();
        assert_eq!(configured_preset("minecraft:nope", set, &mut report), None);
        let text = report.to_string();
        assert!(text.contains("minecraft:worldgen/world_preset"), "{text}");
        assert!(text.contains("minecraft:nope"), "{text}");

        let presets = set.registry::<keys::WorldPreset>().unwrap();
        let mut report = LoadReport::new();
        for (name, expected) in [
            ("beta", "minecraft:beta"),
            ("minecraft:normal", "minecraft:normal"),
        ] {
            assert_eq!(
                configured_preset(name, set, &mut report),
                presets.by_name(expected),
                "{name}"
            );
        }
        assert!(report.is_empty(), "{report}");
    }
}
