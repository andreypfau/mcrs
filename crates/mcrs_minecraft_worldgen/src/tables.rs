use crate::bevy::{
    CarverConfigAsset, FeatureAsset, NoiseGeneratorSettingsAsset, PlacedFeatureAsset,
    ProcessorListAsset, StructureAsset, StructureSetAsset, TemplatePoolAsset,
};
use bevy_asset::{Asset, AssetServer, Assets};
use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistrySet, UnknownEntry};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_structure::StructureSet;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Resource, Clone)]
pub struct WorldgenTables {
    pub carvers: Entries<keys::Carver, Option<CarverConfig>>,
    pub features: Entries<keys::Feature, Option<FeatureAsset>>,
    pub placed_features: Entries<keys::PlacedFeature, Option<PlacedFeatureAsset>>,
    pub processor_lists: Entries<keys::ProcessorList, Option<StructureProcessorList>>,
    pub structures: Entries<keys::Structure, Option<StructureAsset>>,
    pub structure_sets: Entries<keys::StructureSet, Option<StructureSet>>,
    pub template_pools: Entries<keys::TemplatePool, Option<TemplatePoolAsset>>,
    pub noise_settings: Entries<keys::NoiseSettings, Option<NoiseGeneratorSettingsAsset>>,
}

impl SharedResource for WorldgenTables {
    fn shares_with(&self, other: &Self) -> bool {
        self.carvers.shares_with(&other.carvers)
            && self.features.shares_with(&other.features)
            && self.placed_features.shares_with(&other.placed_features)
            && self.processor_lists.shares_with(&other.processor_lists)
            && self.structures.shares_with(&other.structures)
            && self.structure_sets.shares_with(&other.structure_sets)
            && self.template_pools.shares_with(&other.template_pools)
            && self.noise_settings.shares_with(&other.noise_settings)
    }
}

fn empty<R: RegistryKey, T>() -> Entries<R, Option<T>> {
    let registry = Registry::<R>::new([], []).expect("a registry of no entries");
    Entries::new(&registry, Vec::new()).expect("no values for no entries")
}

impl Default for WorldgenTables {
    fn default() -> Self {
        WorldgenTables {
            carvers: empty(),
            features: empty(),
            placed_features: empty(),
            processor_lists: empty(),
            structures: empty(),
            structure_sets: empty(),
            template_pools: empty(),
            noise_settings: empty(),
        }
    }
}

#[derive(Debug, Error)]
pub enum TableError {
    #[error(transparent)]
    Unknown(#[from] UnknownEntry),
    #[error("registry {registry} holds {name}, but no value of it loaded")]
    Absent {
        registry: ResourceLocation,
        name: String,
    },
}

pub fn lookup<'a, R: RegistryKey, T>(
    registry: &Registry<R>,
    entries: &'a Entries<R, Option<T>>,
    name: &str,
) -> Result<&'a T, TableError> {
    lookup_id(registry, entries, registry.require(name)?)
}

pub fn lookup_id<'a, R: RegistryKey, T>(
    registry: &Registry<R>,
    entries: &'a Entries<R, Option<T>>,
    id: Id<R>,
) -> Result<&'a T, TableError> {
    entries
        .get(id)
        .and_then(Option::as_ref)
        .ok_or_else(|| TableError::Absent {
            registry: R::KEY.into(),
            name: registry
                .key(id)
                .map_or_else(|| format!("{id:?}"), ToString::to_string),
        })
}

/// The path a registry entry's asset loads from, which is also the path
/// `build_worldgen_tables` finds the loaded asset at.
pub fn asset_path<R: RegistryKey>(name: &ResourceLocation) -> String {
    format!(
        "{}/{}/{}.json",
        name.namespace(),
        R::KEY.path(),
        name.path()
    )
}

pub fn named<'a, R: RegistryKey, T, V>(
    set: &RegistrySet,
    entries: &'a Entries<R, Option<T>>,
    value: impl Fn(&'a T) -> &'a V,
) -> BTreeMap<ResourceLocation, &'a V> {
    let registry = declared::<R>(set);
    registry
        .ids()
        .filter_map(|id| {
            let name = registry.key(id).expect("an id of the registry has a name");
            Some((name.clone(), value(entries.get(id)?.as_ref()?)))
        })
        .collect()
}

fn declared<R: RegistryKey>(set: &RegistrySet) -> Registry<R> {
    set.registry::<R>()
        .unwrap_or_else(|| panic!("{} is not a loaded registry", R::KEY))
}

fn column<R: RegistryKey, A: Asset, T>(
    set: &RegistrySet,
    asset_server: &AssetServer,
    assets: &Assets<A>,
    value: impl Fn(&A) -> T,
) -> Entries<R, Option<T>> {
    let registry = declared::<R>(set);
    let values = registry
        .ids()
        .map(|id| {
            let name = registry.key(id).expect("an id of the registry has a name");
            let path = asset_path::<R>(name);
            // A handle that exists without an asset is a file that failed to
            // load; a name nothing referenced has no handle and nothing to report.
            let handle = asset_server.get_handle::<A>(path)?;
            let Some(asset) = assets.get(&handle) else {
                tracing::error!(registry = %R::KEY, %name, "the asset of this entry did not load");
                return None;
            };
            Some(value(asset))
        })
        .collect();
    Entries::new(&registry, values).expect("one value for every id")
}

#[allow(clippy::too_many_arguments)]
pub fn build_worldgen_tables(
    mut commands: Commands,
    set: Res<RegistrySet>,
    asset_server: Res<AssetServer>,
    carvers: Res<Assets<CarverConfigAsset>>,
    features: Res<Assets<FeatureAsset>>,
    placed_features: Res<Assets<PlacedFeatureAsset>>,
    processor_lists: Res<Assets<ProcessorListAsset>>,
    structures: Res<Assets<StructureAsset>>,
    structure_sets: Res<Assets<StructureSetAsset>>,
    template_pools: Res<Assets<TemplatePoolAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
) {
    let server = &*asset_server;
    let tables = WorldgenTables {
        carvers: column(&set, server, &carvers, |asset| asset.config.clone()),
        features: column(&set, server, &features, Clone::clone),
        placed_features: column(&set, server, &placed_features, Clone::clone),
        processor_lists: column(&set, server, &processor_lists, |asset| asset.list.clone()),
        structures: column(&set, server, &structures, Clone::clone),
        structure_sets: column(&set, server, &structure_sets, |asset| asset.set.clone()),
        template_pools: column(&set, server, &template_pools, Clone::clone),
        noise_settings: column(&set, server, &noise_settings, Clone::clone),
    };
    tracing::info!(
        carvers = tables.carvers.as_slice().len(),
        features = tables.features.as_slice().len(),
        placed_features = tables.placed_features.as_slice().len(),
        processor_lists = tables.processor_lists.as_slice().len(),
        noise_settings = tables.noise_settings.as_slice().len(),
        structures = tables.structures.as_slice().len(),
        structure_sets = tables.structure_sets.as_slice().len(),
        template_pools = tables.template_pools.as_slice().len(),
        "built the worldgen tables"
    );
    commands.insert_resource(tables);
}
