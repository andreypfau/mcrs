use crate::world::generate::routers::DimensionBiomeSources;
use crate::world::generate::structures::{DimensionStructures, build_dimension_structures};
use crate::world_options::WorldSeed;
use bevy_app::{App, Plugin};
use bevy_asset::Assets;
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_biome::parameter_list::parameter_lists_of;
use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_biome_file::BiomeGenerationSettings;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{HolderSet, Registry, RegistrySet, Tags};
use mcrs_minecraft_worldgen::bevy::TemplateAsset;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, named};
use mcrs_minecraft_worldgen_feature::compile::{LoadedFeatures, build_feature_steps};
use mcrs_minecraft_worldgen_feature::proto::FeatureStepList;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;
use mcrs_minecraft_worldgen_generator::feature_program::FeatureProgram;
use mcrs_minecraft_worldgen_generator::features::{FeatureTables, possible_biomes};
use mcrs_minecraft_worldgen_generator::ids::SurvivalIds;
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
                .before(crate::world::enqueue_dim_spawns),
        );
    }
}

/// A biome's decoration steps in the form the feature compiler reads.
fn decoration_steps(
    steps: &[HolderSet<PlacedFeature>],
    placed: &Registry<PlacedFeature>,
    tags: &Tags<PlacedFeature>,
) -> Vec<FeatureStepList> {
    steps
        .iter()
        .map(|step| {
            step.ids(tags)
                .map(|id| {
                    Holder::Reference(
                        placed
                            .name(id)
                            .expect("an id of the registry has a name")
                            .clone(),
                    )
                })
                .collect()
        })
        .collect()
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
    registries: Res<RegistrySet>,
    survival: Res<Resolved<SurvivalIds>>,
) {
    let Some(sources) = sources else { return };

    let provider_registry = registries
        .registry::<mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider>()
        .expect("the data pack loader parses minecraft:worldgen/block_state_provider");
    let providers = registries
        .entries::<mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider, DirectBlockStateProvider>()
        .expect("the data pack loader parses minecraft:worldgen/block_state_provider");

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
        block_state_providers: provider_registry
            .ids()
            .map(|id| {
                let name = provider_registry
                    .name(id)
                    .expect("an id of the registry has a name");
                (name.clone(), providers[id].clone())
            })
            .collect(),
    };

    let placed_names = registries
        .registry::<PlacedFeature>()
        .expect("the data pack loader declares minecraft:worldgen/placed_feature");
    let placed_tags = registries
        .tags::<PlacedFeature>()
        .expect("the data pack loader builds the placed feature tags");
    let biome_registry = registries
        .registry::<Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let biomes = registries
        .entries::<Biome, Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let generation = registries
        .entries::<Biome, BiomeGenerationSettings>()
        .expect("the data pack loader splits minecraft:worldgen/biome");
    let by_id: BTreeMap<ResourceLocation, (&Biome, &BiomeGenerationSettings)> = biome_registry
        .ids()
        .map(|id| {
            let name = biome_registry
                .name(id)
                .expect("an id of the registry has a name");
            (name.clone(), (&biomes[id], &generation[id]))
        })
        .collect();

    let climate: BTreeMap<ResourceLocation, BiomeClimate> = by_id
        .iter()
        .map(|(id, (biome, _))| {
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

    let parameter_lists = parameter_lists_of(&registries);
    let mut programs = DimensionFeaturePrograms::default();
    for (dimension, source) in &sources.0 {
        let biome_order = possible_biomes(source, &biome_registry, &parameter_lists);
        let mut steps = Vec::with_capacity(biome_order.len());
        for id in &biome_order {
            match by_id.get(id) {
                Some((_, generation)) => steps.push(decoration_steps(
                    &generation.features,
                    &placed_names,
                    &placed_tags,
                )),
                // Dropping the biome would shorten the sort's input, and the
                // sort's positions are the seeds, so a missing definition is a
                // different world rather than one biome's worth less.
                None => panic!("{dimension}: the biome {id} has no loaded definition"),
            }
        }
        let entries: Vec<&[FeatureStepList]> = steps.iter().map(Vec::as_slice).collect();

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
            &registries,
            seed.0 as i64,
            structures
                .as_ref()
                .and_then(|structures| structures.0.get(dimension))
                .map(|tables| &*tables.frozen),
            survival.clone(),
        )
        .unwrap_or_else(|error| {
            panic!("{dimension}: the feature program does not resolve: {error}")
        });
        programs.0.insert(dimension.clone(), Arc::new(program));
    }
    commands.insert_resource(programs);
}
