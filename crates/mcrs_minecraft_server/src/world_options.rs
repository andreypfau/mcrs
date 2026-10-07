use crate::WorldSave;
use crate::loaded::Loaded;
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use mcrs_minecraft_world::LoadedRegistryAssets;
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_world::registries::refuse;
use mcrs_minecraft_world::save::read_world_gen_settings;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_worldgen::bevy::NoiseGeneratorSettingsAsset;
use mcrs_minecraft_worldgen::tables::asset_path;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use std::sync::Arc;
use tracing::{error, info};

/// The dimension list the server plugin was given, which replaces the save's and the preset's.
#[derive(Resource, Clone)]
pub(crate) struct PluginDimensions(pub Dimensions);

#[derive(Resource)]
pub(crate) struct WorldPresetName(pub String);

pub(crate) fn bake_dimensions(
    mut commands: Commands,
    set: Res<RegistrySet>,
    save: Option<Res<WorldSave>>,
    given: Option<Res<PluginDimensions>>,
    preset: Res<WorldPresetName>,
    mut seed: ResMut<WorldSeed>,
) {
    let mut report = LoadReport::new();
    let saved = save.map(|save| {
        let settings = read_world_gen_settings(&save.0, &set).unwrap_or_else(|err| panic!("{err}"));
        seed.0 = settings.seed as u64;
        settings.dimensions
    });
    let (source, list) = if let Some(given) = given {
        ("plugin".to_owned(), bake_list(&given.0, &set, &mut report))
    } else if let Some(saved) = saved {
        ("save".to_owned(), bake(&saved, &set, &mut report))
    } else {
        let name = &preset.0;
        let Some(preset) = report
            .registry(&set, mcrs_minecraft_world::keys::WORLD_PRESET)
            .and_then(|registry| report.require_by_name(&registry, name))
        else {
            refuse(&report)
        };
        let base = &set.loaded_entries::<WorldPreset, WorldPreset>()[preset].dimensions;
        (name.clone(), bake(base, &set, &mut report))
    };
    let Some(list) = list else { refuse(&report) };

    info!(%source, dimensions = list.len(), "dimension list");
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
    let settings = set.loaded_registry::<NoiseGeneratorSettings>();
    for (_, entry) in dimensions.iter() {
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let Some(name) = settings.name(generator.settings) else {
            error!(id = ?generator.settings, "the dimension names noise settings the registry does not number");
            continue;
        };
        let handle = asset_server.load::<NoiseGeneratorSettingsAsset>(asset_path(
            &mcrs_minecraft_worldgen_density::keys::NOISE_SETTINGS.location(),
            name,
        ));
        loaded.push(handle.untyped());
    }
}

/// Every dimension the world spawns, in spawn order. Written once at `Startup`.
#[derive(Resource, Clone)]
pub struct DimensionList {
    keys: Arc<[ResourceKey<Dimension>]>,
    entries: Arc<[DimensionEntry]>,
}

impl DimensionList {
    pub fn new(list: Vec<(ResourceKey<Dimension>, DimensionEntry)>) -> Self {
        let (keys, entries): (Vec<_>, Vec<_>) = list.into_iter().unzip();
        DimensionList {
            keys: keys.into(),
            entries: entries.into(),
        }
    }

    pub fn keys(&self) -> &Arc<[ResourceKey<Dimension>]> {
        &self.keys
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceKey<Dimension>, &DimensionEntry)> {
        self.keys.iter().zip(self.entries.iter())
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// The seed every dimension's noise router is compiled against. One writer in
/// the host; the routers carry it into the sub-apps.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldSeed(pub u64);
