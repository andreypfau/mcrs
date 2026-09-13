use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_app::{App, Plugin};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry::snapshot::rl_from_asset_path;
use mcrs_minecraft_core::{DynTagRegistry, RegistrySnapshot};
use mcrs_minecraft_decoration::feature::terrain_skin::BiomeClimate;
use mcrs_minecraft_world::biome::overworld_preset::{
    nether_parameter_list, overworld_parameter_list,
};
use mcrs_minecraft_world::biome::source::BiomeSource;
use mcrs_minecraft_world::biome::{Biome, TemperatureModifier};
use mcrs_minecraft_world::block::Block as VanillaBlock;
use mcrs_minecraft_world::block::Fluid;
use mcrs_minecraft_world::block::definition::Blocks;
use mcrs_minecraft_worldgen::bevy::{FeatureAsset, PlacedFeatureAsset};
use mcrs_minecraft_worldgen::feature::compile::{
    FeatureSteps, LoadedFeatures, build_feature_steps,
};

use crate::configuration::WorldSeed;
use crate::world::generate::feature_program::FeatureProgram;
use crate::world::generate::routers::DimensionBiomeSources;

/// `TheEndBiomeSource` lists its five biomes in this order, and that order is
/// the input of the sort.
const END_BIOMES: [&str; 5] = [
    "minecraft:the_end",
    "minecraft:end_highlands",
    "minecraft:end_midlands",
    "minecraft:small_end_islands",
    "minecraft:end_barrens",
];

/// One dimension's sorted feature tables: what [`FeatureProgram::build`]
/// resolves into numbers.
#[derive(Clone)]
pub struct FeatureTables {
    pub features: FeatureSteps,
    /// The biome source's own order, which decides the sort.
    pub biome_order: Vec<ResourceLocation>,
    /// What each biome's climate says about freezing. Only the parsed biome
    /// carries it, and the frozen registry the feature program builds against
    /// holds NBT, so it is read here and carried rather than resolved there.
    pub climate: BTreeMap<ResourceLocation, BiomeClimate>,
}

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
            build_dimension_features.before(crate::world::enqueue_dim_spawns_from_preset),
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

/// The biomes a source can answer with, in the source's own order and without
/// repeats — the input the feature order is defined against.
pub(crate) fn possible_biomes(
    source: &BiomeSource,
    named: impl Fn(&bevy_asset::Handle<Biome>) -> Option<ResourceLocation>,
) -> Vec<ResourceLocation> {
    let listed: Vec<ResourceLocation> = match source {
        BiomeSource::MultiNoise(multi) => match (&multi.biomes, &multi.preset) {
            (Some(entries), _) => entries.iter().map(|entry| entry.location.clone()).collect(),
            (None, Some(preset)) => match preset.as_str() {
                "minecraft:overworld" => preset_biomes(&overworld_parameter_list()),
                "minecraft:nether" => preset_biomes(&nether_parameter_list()),
                other => panic!("no biome list for the multi-noise preset {other}"),
            },
            (None, None) => panic!("a multi-noise source names neither biomes nor a preset"),
        },
        BiomeSource::TheEnd => END_BIOMES
            .iter()
            .filter_map(|id| ResourceLocation::parse(id).ok())
            .collect(),
        BiomeSource::Fixed { biome_id, .. } => vec![biome_id.clone()],
        BiomeSource::Checkerboard { biomes, .. } => biomes.iter().filter_map(named).collect(),
        BiomeSource::Beta { land_biome_ids, .. } => land_biome_ids.to_vec(),
    };

    let mut seen = std::collections::HashSet::new();
    listed
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn preset_biomes(
    list: &mcrs_minecraft_world::biome::climate::ParameterList<&'static str>,
) -> Vec<ResourceLocation> {
    list.values()
        .iter()
        .filter_map(|(_, biome)| ResourceLocation::parse(biome).ok())
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
    asset_server: Res<AssetServer>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    block_tags: Option<Res<DynTagRegistry<VanillaBlock>>>,
    fluid_tags: Option<Res<DynTagRegistry<Fluid>>>,
    biome_registry: Res<RegistrySnapshot<Biome>>,
) {
    let Some(sources) = sources else { return };

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
        )
        .unwrap_or_else(|error| {
            panic!("{dimension}: the feature program does not resolve: {error}")
        });
        programs.0.insert(dimension.clone(), Arc::new(program));
    }
    commands.insert_resource(programs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Buf;
    use mcrs_minecraft_worldgen::corpus::dump_string;
    use std::path::Path;

    /// The reference's own `possibleBiomes`, per source, from the dump the
    /// feature-order oracle wrote.
    fn dumped_biomes() -> BTreeMap<String, Vec<String>> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mcrs_minecraft_worldgen/tests/fixtures/vanilla/feature_steps.bin");
        let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut r: &[u8] = &data;
        r.copy_to_bytes(8);
        assert_eq!(r.get_i32_le(), 1, "unsupported oracle format version");
        assert_eq!(r.get_i32_le(), 5015, "the dump is from another snapshot");

        let mut sources = BTreeMap::new();
        for _ in 0..r.get_i32_le() {
            let id = dump_string(&mut r);
            let biomes: Vec<String> = (0..r.get_i32_le()).map(|_| dump_string(&mut r)).collect();
            let steps = r.get_i32_le();
            for _ in 0..steps {
                for _ in 0..r.get_i32_le() {
                    dump_string(&mut r);
                }
            }
            for _ in 0..biomes.len() as i32 * steps {
                let skip = r.get_i32_le() as usize;
                r.copy_to_bytes(skip);
            }
            sources.insert(id, biomes);
        }
        assert!(!r.has_remaining(), "trailing bytes in the dump");
        sources
    }

    fn order(source: &BiomeSource) -> Vec<String> {
        possible_biomes(source, |_| None)
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    fn preset(name: &str) -> BiomeSource {
        BiomeSource::MultiNoise(mcrs_minecraft_world::biome::source::MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse(name).unwrap()),
            biomes: None,
        })
    }

    #[test]
    fn every_source_answers_the_biomes_the_reference_does_in_its_order() {
        let dumped = dumped_biomes();
        assert_eq!(
            order(&preset("minecraft:overworld")),
            dumped["minecraft:overworld"]
        );
        assert_eq!(
            order(&preset("minecraft:nether")),
            dumped["minecraft:the_nether"]
        );
        assert_eq!(order(&BiomeSource::TheEnd), dumped["minecraft:the_end"]);
    }
}
