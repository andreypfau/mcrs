use crate::world::generate::routers::DimensionBiomeSources;
use crate::world::generate::structures::{DimensionStructures, build_dimension_structures};
use crate::world_options::WorldSeed;
use bevy_app::{App, Plugin};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::snapshot::rl_from_asset_path;
use mcrs_minecraft_assets::{DynTagRegistry, RegistrySnapshot};
use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_block::Block as VanillaBlock;
use mcrs_minecraft_block::Fluid;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen::bevy::{
    BlockStateProviderAsset, FeatureAsset, PlacedFeatureAsset, ProcessorListAsset, TemplateAsset,
    TemplatePoolAsset,
};
use mcrs_minecraft_worldgen_feature::compile::{LoadedFeatures, build_feature_steps};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;
use mcrs_minecraft_worldgen_generator::feature_program::FeatureProgram;
use mcrs_minecraft_worldgen_generator::features::{FeatureTables, possible_biomes};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Resource, Default, Clone)]
pub struct DimensionFeaturePrograms(pub BTreeMap<ResourceLocation, Arc<FeatureProgram>>);

pub struct FeaturePlugin;

impl Plugin for FeaturePlugin {
    fn build(&self, app: &mut App) {
        // Beside the routers: the seed is known, the block and tag registries
        // are frozen, and no sub-app exists yet to miss the program.
        //
        // Once, and never again: the program is what a column's blocks are a
        // function of, so rebuilding it under a running world would make the
        // same chunk generate differently before and after a reload. A changed
        // feature asset needs a restart, by design.
        app.add_systems(
            OnEnter(AppState::Playing),
            build_dimension_features
                .after(build_dimension_structures)
                .before(crate::world::enqueue_dim_spawns_from_preset),
        );
    }
}

pub(crate) fn registry_of<A: bevy_asset::Asset, T: Clone>(
    assets: &Assets<A>,
    asset_server: &AssetServer,
    folder: &str,
    value: impl Fn(&A) -> &T,
) -> BTreeMap<ResourceLocation, T> {
    assets
        .iter()
        .filter_map(|(asset_id, asset)| {
            let path = asset_server.get_path(asset_id)?;
            let id = rl_from_asset_path(path.path(), folder)?;
            Some((id, value(asset).clone()))
        })
        .collect()
}

/// Resolve every dimension's biomes into the ordered feature steps, and those
/// into the program its columns run.
///
/// Every half is a loaded asset: which features a biome carries comes from the
/// biome JSON, what each one is from the two feature registries, and what it
/// writes from the block definitions and tags, so a datapack that changes any
/// of them is picked up here. A name that resolves to nothing stops the server
/// naming the asset.
#[allow(clippy::too_many_arguments)]
fn build_dimension_features(
    mut commands: Commands,
    sources: Option<Res<DimensionBiomeSources>>,
    biomes: Res<Assets<Biome>>,
    features: Res<Assets<FeatureAsset>>,
    placed_features: Res<Assets<PlacedFeatureAsset>>,
    pools: Res<Assets<TemplatePoolAsset>>,
    templates: Res<Assets<TemplateAsset>>,
    processor_lists: Res<Assets<ProcessorListAsset>>,
    block_state_providers: Res<Assets<BlockStateProviderAsset>>,
    structures: Option<Res<DimensionStructures>>,
    asset_server: Res<AssetServer>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    block_tags: Option<Res<DynTagRegistry<VanillaBlock>>>,
    fluid_tags: Option<Res<DynTagRegistry<Fluid>>>,
    biome_registry: Res<RegistrySnapshot<Biome>>,
) {
    let Some(sources) = sources else { return };

    // A template is `structure/<id>.nbt`, which `registry_of` cannot name, so
    // the ids come off the handles the feature, placed-feature and pool assets
    // declared: a pool can inline a template feature.
    // ponytail: every pool template is cloned for the few an inline template
    // feature might name; upgrade = walk the pool elements for template
    // feature nodes and take only theirs.
    let template_values = features
        .iter()
        .map(|(_, asset)| &asset.deps)
        .chain(placed_features.iter().map(|(_, asset)| &asset.deps))
        .chain(pools.iter().map(|(_, asset)| &asset.deps))
        .flat_map(|deps| deps.templates.iter())
        .filter_map(|(id, handle)| Some((id.clone(), templates.get(handle)?.template.clone())))
        .collect();
    let loaded = LoadedFeatures {
        features: registry_of(&features, &asset_server, "worldgen/feature", |asset| {
            &asset.feature
        }),
        placed_features: registry_of(
            &placed_features,
            &asset_server,
            "worldgen/placed_feature",
            |asset| &asset.placed_feature,
        ),
        templates: template_values,
        processor_lists: registry_of(
            &processor_lists,
            &asset_server,
            "worldgen/processor_list",
            |asset| &asset.list,
        ),
        block_state_providers: registry_of(
            &block_state_providers,
            &asset_server,
            "worldgen/block_state_provider",
            |asset| &asset.provider,
        ),
    };

    let by_id: BTreeMap<ResourceLocation, &Biome> = biomes
        .iter()
        .filter_map(|(asset_id, biome)| {
            let path = asset_server.get_path(asset_id)?;
            Some((rl_from_asset_path(path.path(), "worldgen/biome")?, biome))
        })
        .collect();

    let climate: BTreeMap<ResourceLocation, BiomeClimate> = by_id
        .iter()
        .map(|(id, biome)| {
            (
                id.clone(),
                BiomeClimate {
                    base_temperature: biome.temperature,
                    frozen: biome.temperature_modifier == Some(TemperatureModifier::Frozen),
                    has_precipitation: biome.has_precipitation,
                },
            )
        })
        .collect();

    let mut programs = DimensionFeaturePrograms::default();
    for (dimension, source) in &sources.0 {
        let biome_order = possible_biomes(source, |handle| {
            rl_from_asset_path(asset_server.get_path(handle.id())?.path(), "worldgen/biome")
        });
        let mut entries = Vec::with_capacity(biome_order.len());
        for id in &biome_order {
            match by_id.get(id) {
                Some(biome) => entries.push(&biome.features[..]),
                // Dropping the biome would shorten the sort's input, and the
                // sort's positions are the seeds, so a missing definition is a
                // different world rather than one biome's worth less.
                None => panic!("{dimension}: the biome {id} has no loaded definition"),
            }
        }

        let built = build_feature_steps(&entries, &loaded).unwrap_or_else(|error| {
            panic!("{dimension}: the feature steps do not resolve: {error}")
        });
        tracing::info!(
            %dimension,
            steps = built.steps.len(),
            features = built.steps.iter().map(Vec::len).sum::<usize>(),
            "resolved the feature steps"
        );
        let tables = FeatureTables {
            features: built,
            biome_order,
            climate: climate.clone(),
        };
        let program = FeatureProgram::build(
            &tables,
            &loaded,
            &blocks.0,
            block_tags.as_deref(),
            fluid_tags.as_deref(),
            &biome_registry,
            seed.0 as i64,
            structures
                .as_ref()
                .and_then(|structures| structures.0.get(dimension))
                .map(|tables| &*tables.frozen),
        )
        .unwrap_or_else(|error| {
            panic!("{dimension}: the feature program does not resolve: {error}")
        });
        programs.0.insert(dimension.clone(), Arc::new(program));
    }
    commands.insert_resource(programs);
}
