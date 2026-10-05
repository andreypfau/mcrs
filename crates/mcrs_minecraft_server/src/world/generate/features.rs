use crate::world::generate::routers::DimensionBiomeSources;
use crate::world::generate::structures::{DimensionStructures, build_dimension_structures};
use crate::world_options::WorldSeed;
use bevy_app::{App, Plugin};
use bevy_asset::Assets;
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::DynTagRegistry;
use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_keys::Fluid;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen::bevy::TemplateAsset;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, named};
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

/// Resolve every dimension's biomes into the ordered feature steps, and those
/// into the program its columns run.
///
/// Every half is a loaded value: which features a biome carries comes from the
/// biome JSON, what each one is from the feature tables, and what it writes
/// from the block definitions and tags, so a datapack that changes any of them
/// is picked up here. A name that resolves to nothing stops the server naming
/// the entry.
#[allow(clippy::too_many_arguments)]
fn build_dimension_features(
    mut commands: Commands,
    sources: Option<Res<DimensionBiomeSources>>,
    tables: Res<WorldgenTables>,
    templates: Res<Assets<TemplateAsset>>,
    structures: Option<Res<DimensionStructures>>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    block_tags: Option<Res<DynTagRegistry<Block>>>,
    fluid_tags: Option<Res<DynTagRegistry<Fluid>>>,
    registries: Res<RegistrySet>,
) {
    let Some(sources) = sources else { return };

    let features = named(&registries, &tables.features, |asset| asset);
    let placed_features = named(&registries, &tables.placed_features, |asset| asset);
    let pools = named(&registries, &tables.template_pools, |asset| asset);

    // A template is `structure/<id>.nbt`, which is no registry, so the ids come
    // off the handles the feature, placed-feature and pool values declared: a
    // pool can inline a template feature.
    // chisle: every pool template is cloned for the few an inline template
    // feature might name; upgrade = walk the pool elements for template
    // feature nodes and take only theirs.
    let template_values = features
        .values()
        .map(|asset| &asset.deps)
        .chain(placed_features.values().map(|asset| &asset.deps))
        .chain(pools.values().map(|asset| &asset.deps))
        .flat_map(|deps| deps.templates.iter())
        .filter_map(|(id, handle)| Some((id.clone(), templates.get(handle)?.template.clone())))
        .collect();
    let loaded = LoadedFeatures {
        features: features
            .iter()
            .map(|(id, asset)| (id.clone(), asset.feature.clone()))
            .collect(),
        placed_features: placed_features
            .iter()
            .map(|(id, asset)| (id.clone(), asset.placed_feature.clone()))
            .collect(),
        templates: template_values,
        processor_lists: named(&registries, &tables.processor_lists, |list| list)
            .into_iter()
            .map(|(id, list)| (id, list.clone()))
            .collect(),
        block_state_providers: named(&registries, &tables.block_state_providers, |provider| {
            provider
        })
        .into_iter()
        .map(|(id, provider)| (id, provider.clone()))
        .collect(),
    };

    let biome_registry = registries
        .registry::<keys::Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let biomes = registries
        .entries::<keys::Biome, Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let by_id: BTreeMap<ResourceLocation, &Biome> = biome_registry
        .ids()
        .map(|id| {
            let name = biome_registry
                .key(id)
                .expect("an id of the registry has a name");
            (name.clone(), &biomes[id])
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
        let biome_order = possible_biomes(source, &biome_registry);
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
