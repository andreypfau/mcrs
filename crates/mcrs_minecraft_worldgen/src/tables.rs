use crate::bevy::{
    CarverConfigAsset, FeatureAsset, PlacedFeatureAsset, ProcessorListAsset, StructureAsset,
    StructureSetAsset, TemplatePoolAsset,
};
use bevy_asset::{Asset, AssetServer, Assets};
use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistrySet};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::Feature;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_structure::Structure;
use mcrs_minecraft_worldgen_structure::StructureSet;
use std::any::type_name;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Resource, Clone)]
pub struct WorldgenTables {
    pub carvers: Entries<CarverConfig, Option<CarverConfig>>,
    pub features: Entries<Feature, Option<FeatureAsset>>,
    pub placed_features: Entries<PlacedFeature, Option<PlacedFeatureAsset>>,
    pub processor_lists: Entries<StructureProcessorList, Option<StructureProcessorList>>,
    pub structures: Entries<Structure, Option<StructureAsset>>,
    pub structure_sets: Entries<StructureSet, Option<StructureSet>>,
    pub template_pools: Entries<TemplatePool, Option<TemplatePoolAsset>>,
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
    }
}

fn empty<R, T>(key: RegistryKey<R>) -> Entries<R, Option<T>> {
    let registry = Registry::new(key, []).expect("a registry of no entries");
    Entries::new(&registry, Vec::new()).expect("no values for no entries")
}

impl Default for WorldgenTables {
    fn default() -> Self {
        WorldgenTables {
            carvers: empty(mcrs_minecraft_worldgen_carver::keys::CARVER),
            features: empty(mcrs_minecraft_worldgen_feature::keys::FEATURE),
            placed_features: empty(mcrs_minecraft_worldgen_feature::keys::PLACED_FEATURE),
            processor_lists: empty(mcrs_minecraft_worldgen_feature::keys::PROCESSOR_LIST),
            structures: empty(mcrs_minecraft_worldgen_structure::keys::STRUCTURE),
            structure_sets: empty(mcrs_minecraft_worldgen_structure::keys::STRUCTURE_SET),
            template_pools: empty(mcrs_minecraft_worldgen_feature::keys::TEMPLATE_POOL),
        }
    }
}

#[derive(Debug, Error)]
pub enum TableError {
    #[error("registry {registry} holds {name}, but no value of it loaded")]
    Absent {
        registry: ResourceLocation,
        name: String,
    },
}

pub fn lookup_id<'a, R: 'static, T>(
    registry: &Registry<R>,
    entries: &'a Entries<R, Option<T>>,
    id: Id<R>,
) -> Result<&'a T, TableError> {
    entries
        .get(id)
        .and_then(Option::as_ref)
        .ok_or_else(|| TableError::Absent {
            registry: registry.table().registry().clone(),
            name: registry
                .name(id)
                .map_or_else(|| format!("{id:?}"), ToString::to_string),
        })
}

/// The path a registry entry's asset loads from, which is also the path
/// `build_worldgen_tables` finds the loaded asset at.
pub fn asset_path<S: AsRef<str>>(
    registry: &ResourceLocation<S>,
    name: &ResourceLocation,
) -> String {
    format!(
        "{}/{}/{}.json",
        name.namespace(),
        registry.path(),
        name.path()
    )
}

pub fn named<'a, R: 'static, T>(
    set: &RegistrySet,
    entries: &'a Entries<R, Option<T>>,
) -> BTreeMap<ResourceLocation, &'a T> {
    let registry = declared::<R>(set);
    registry
        .ids()
        .filter_map(|id| {
            let name = registry.name(id).expect("an id of the registry has a name");
            Some((name.clone(), entries.get(id)?.as_ref()?))
        })
        .collect()
}

fn declared<R: 'static>(set: &RegistrySet) -> Registry<R> {
    set.registry::<R>()
        .unwrap_or_else(|| panic!("the registry of {} is not loaded", type_name::<R>()))
}

fn column<R: 'static, A: Asset, T>(
    set: &RegistrySet,
    asset_server: &AssetServer,
    assets: &Assets<A>,
    value: impl Fn(&A) -> T,
) -> Entries<R, Option<T>> {
    let registry = declared::<R>(set);
    let values = registry
        .ids()
        .map(|id| {
            let name = registry.name(id).expect("an id of the registry has a name");
            let path = asset_path(registry.table().registry(), name);
            // A handle that exists without an asset is a file that failed to
            // load; a name nothing referenced has no handle and nothing to report.
            let handle = asset_server.get_handle::<A>(path)?;
            let Some(asset) = assets.get(&handle) else {
                tracing::error!(registry = %registry.table().registry(), %name, "the asset of this entry did not load");
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
    };
    tracing::info!(
        carvers = tables.carvers.as_slice().len(),
        features = tables.features.as_slice().len(),
        placed_features = tables.placed_features.as_slice().len(),
        processor_lists = tables.processor_lists.as_slice().len(),
        structures = tables.structures.as_slice().len(),
        structure_sets = tables.structure_sets.as_slice().len(),
        template_pools = tables.template_pools.as_slice().len(),
        "built the worldgen tables"
    );
    commands.insert_resource(tables);
}
