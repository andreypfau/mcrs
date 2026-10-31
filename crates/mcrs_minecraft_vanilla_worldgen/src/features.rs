use crate::structures::{DimensionStructures, build_dimension_structures};
use crate::template_asset::{TemplateAsset, named_templates};
use crate::template_handle::template_handle;
use bevy_app::{App, AppExit, Plugin};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, MessageWriter, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_biome::TemperatureModifier;
use mcrs_minecraft_biome::parameter_list::parameter_lists_of;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_level::world::dimension_list::DimensionList;
use mcrs_minecraft_level::world::sub_app::DimSpawnEnqueue;
use mcrs_minecraft_registries::{
    CurrentRegistries, IdSet, Named, Namer, Registries, Registry, Resolve, Tags,
};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen_feature::compile::{LoadedFeatures, build_feature_steps};
use mcrs_minecraft_worldgen_feature::proto::FeatureStepList;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_feature::template::Template;
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
                .before(DimSpawnEnqueue),
        );
    }
}

/// A biome's decoration steps in the form the feature compiler reads.
fn decoration_steps(
    steps: &[IdSet<mcrs_minecraft_biome::PlacedFeature>],
    registries: &Registries,
    tags: &Tags<mcrs_minecraft_biome::PlacedFeature>,
) -> Vec<FeatureStepList> {
    steps
        .iter()
        .map(|step| {
            step.ids(tags)
                .filter_map(|id| Some(Holder::Reference(Namer::name(registries, id)?.clone())))
                .collect()
        })
        .collect()
}

/// Every placed feature tag by name, with its members by name.
fn placed_feature_tags(
    registries: &Registries,
    tags: &Tags<mcrs_minecraft_biome::PlacedFeature>,
) -> BTreeMap<ResourceLocation, Vec<ResourceLocation>> {
    tags.iter()
        .map(|(tag, name)| {
            let members = tags
                .members(tag)
                .filter_map(|id| Namer::name(registries, id).cloned())
                .collect();
            (name.clone(), members)
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
fn named_providers(
    registries: &Registries,
    providers: &Registry<DirectBlockStateProvider>,
) -> BTreeMap<ResourceLocation, DirectBlockStateProvider<Named>> {
    providers
        .iter()
        .filter_map(|(_, name, provider)| match provider.name(registries) {
            Ok(provider) => Some((name.clone(), provider)),
            Err(error) => {
                tracing::error!(%name, %error, "a block state provider cannot be named");
                None
            }
        })
        .collect()
}

fn loaded_features(
    registries: &Registries,
    templates: BTreeMap<ResourceLocation, Template>,
) -> Option<LoadedFeatures> {
    Some(LoadedFeatures {
        features: registries
            .registry::<Feature>()?
            .iter()
            .map(|(_, name, feature)| (name.clone(), feature.clone()))
            .collect(),
        placed_features: registries
            .registry::<PlacedFeature>()?
            .iter()
            .map(|(_, name, feature)| (name.clone(), feature.clone()))
            .collect(),
        templates,
        processor_lists: registries
            .registry::<StructureProcessorList>()?
            .iter()
            .map(|(_, name, list)| (name.clone(), list.clone()))
            .collect(),
        block_state_providers: named_providers(registries, registries.registry()?),
        placed_feature_tags: placed_feature_tags(registries, &registries.tags()?),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn build_dimension_features(
    mut commands: Commands,
    dimensions: Res<DimensionGenerators>,
    list: Res<DimensionList>,
    asset_server: Res<AssetServer>,
    templates: Res<Assets<TemplateAsset>>,
    structures: Option<Res<DimensionStructures>>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut exit: MessageWriter<AppExit>,
) {
    // chisle: every named template is cloned, though only the templates a
    // template feature places are read from here; upgrade = collect those alone.
    let template_names = match named_templates(&registries) {
        Ok(names) => names,
        Err(error) => {
            tracing::error!("the loaded registries cannot list their templates: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    let template_values = template_names
        .into_iter()
        .filter_map(|id| {
            let handle = template_handle(&asset_server, &id)?;
            Some((id, templates.get(&handle)?.template.clone()))
        })
        .collect();
    let (Some(loaded), Some(placed_tags), Some(biomes)) = (
        loaded_features(&registries, template_values),
        registries.tags::<mcrs_minecraft_biome::PlacedFeature>(),
        registries.registry::<BiomeFile>(),
    ) else {
        tracing::error!(
            "the registries declare no feature, placed feature, processor list, block state provider or biome registry; no dimension gets a feature program"
        );
        return;
    };

    let by_id: BTreeMap<ResourceLocation, &BiomeFile> = biomes
        .iter()
        .map(|(_, name, biome)| (name.clone(), biome))
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

    let parameter_lists = parameter_lists_of(&registries).unwrap_or_else(|| Arc::from(Vec::new()));
    let mut programs = DimensionFeaturePrograms::default();
    for (dimension, generator) in dimensions.iter(&list) {
        let ChunkGenerator::Noise(generator) = generator else {
            continue;
        };
        let dimension = dimension.location();
        let biome_order = possible_biomes(&generator.biome_source, &registries, &parameter_lists);
        let program = (|| -> Result<FeatureProgram, String> {
            // Dropping the biome would shorten the sort's input, and the
            // sort's positions are the seeds, so a missing definition is a
            // different world rather than one biome's worth less.
            let steps = biome_order
                .iter()
                .map(|id| {
                    by_id
                        .get(id)
                        .map(|biome| decoration_steps(&biome.features, &registries, &placed_tags))
                        .ok_or_else(|| format!("the biome {id} has no loaded definition"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let entries: Vec<&[FeatureStepList]> = steps.iter().map(Vec::as_slice).collect();
            let built = build_feature_steps(&entries, &loaded)
                .map_err(|error| format!("the feature steps do not resolve: {error}"))?;
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
            FeatureProgram::build(
                &tables,
                &loaded,
                &blocks.0,
                &registries,
                seed.0 as i64,
                structures
                    .as_ref()
                    .and_then(|structures| structures.0.get(dimension))
                    .map(|tables| &*tables.frozen),
            )
            .map_err(|error| format!("the feature program does not resolve: {error}"))
        })();
        let program = match program {
            Ok(program) => program,
            Err(error) => {
                tracing::error!(%dimension, "{error}");
                exit.write(AppExit::error());
                return;
            }
        };
        programs.0.insert(dimension.clone(), Arc::new(program));
    }
    commands.insert_resource(programs);
}
