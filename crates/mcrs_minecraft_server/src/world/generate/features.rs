use crate::loaded::Loaded;
use crate::world::generate::structures::{DimensionStructures, build_dimension_structures};
use crate::world_options::{DimensionList, WorldSeed};
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
use mcrs_minecraft_registry::{HolderSet, Registry, RegistrySet, Tags};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen::bevy::TemplateAsset;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, named};
use mcrs_minecraft_worldgen_feature::compile::{LoadedFeatures, build_feature_steps};
use mcrs_minecraft_worldgen_feature::proto::FeatureStepList;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
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
    dimensions: Res<DimensionList>,
    tables: Res<WorldgenTables>,
    templates: Res<Assets<TemplateAsset>>,
    structures: Res<DimensionStructures>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    registries: Res<RegistrySet>,
) {
    let provider_registry = registries.loaded_registry::<DirectBlockStateProvider>();
    let providers =
        registries.loaded_entries::<DirectBlockStateProvider, DirectBlockStateProvider>();

    let features = named(&registries, &tables.features);
    let placed_features = named(&registries, &tables.placed_features);
    let pools = named(&registries, &tables.template_pools);

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
        processor_lists: named(&registries, &tables.processor_lists)
            .into_iter()
            .map(|(id, list)| (id, list.clone()))
            .collect(),
        block_state_providers: provider_registry
            .iter()
            .map(|(id, name)| (name.clone(), providers[id].clone()))
            .collect(),
    };

    let placed_names = registries.loaded_registry::<PlacedFeature>();
    let placed_tags = registries.loaded_tags::<PlacedFeature>();
    let biome_registry = registries.loaded_registry::<Biome>();
    let biome_tags = registries.loaded_tags::<Biome>();
    let biomes = registries.loaded_entries::<Biome, Biome>();
    let generation = registries.loaded_entries::<Biome, BiomeGenerationSettings>();
    let by_id: BTreeMap<ResourceLocation, (&Biome, &BiomeGenerationSettings)> = biome_registry
        .iter()
        .map(|(id, name)| (name.clone(), (&biomes[id], &generation[id])))
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
    for (dimension, entry) in dimensions.iter() {
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let dimension = dimension.location();
        let biome_order = possible_biomes(
            &generator.biome_source,
            &biome_registry,
            &biome_tags,
            &parameter_lists,
        );
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
            structures.0.get(dimension).map(|tables| &*tables.frozen),
        )
        .unwrap_or_else(|error| {
            panic!("{dimension}: the feature program does not resolve: {error}")
        });
        programs.0.insert(dimension.clone(), Arc::new(program));
    }
    commands.insert_resource(programs);
}
