use std::sync::Arc;

use bevy_app::SubApp;
use bevy_ecs::world::World;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::heightmap::BlockHeightmapPredicates;
use mcrs_minecraft_level::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_level::world::dimension_list::DimensionList;
use mcrs_minecraft_level::world::world_save::WorldSave;
use mcrs_minecraft_registries::{CurrentRegistries, Diagnostics, Registries};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen_generator::SurfaceIds;
use mcrs_minecraft_worldgen_generator::ids::FillIds;
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use mcrs_minecraft_worldgen_generator::saved::SavedColumns;
use mcrs_minecraft_worldgen_generator::stages::{FillContext, dimension_y_sections};
use tracing::{error, warn};

use crate::chunk_plugin::{ChunkPlugin, SlowColumnThreshold};
use crate::dimension_generators::DimensionGenerators;
use crate::features::DimensionFeaturePrograms;
use crate::modern_carvers::DimensionCarverBiomes;
use crate::routers::DimensionRouters;
use crate::structures::DimensionStructures;

pub(crate) fn build_dimension_world(
    host: &World,
    build: &DimensionBuild<'_>,
    sub_app: &mut SubApp,
) {
    insert_fill_context(host, build, sub_app);
    sub_app.add_plugins(ChunkPlugin);
    sub_app.insert_resource(
        host.get_resource::<SlowColumnThreshold>()
            .copied()
            .unwrap_or_default(),
    );
    sub_app.insert_resource(
        host.get_resource::<DimensionStructures>()
            .cloned()
            .unwrap_or_default(),
    );
}

fn insert_fill_context(host: &World, build: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    let dimension = build.dimension.location();
    let Some(dimension_router) = host
        .get_resource::<DimensionRouters>()
        .and_then(|routers| routers.0.get(dimension))
    else {
        warn!(%dimension, "no noise router for this dimension; it will generate nothing");
        return;
    };
    let biome_source = host
        .get_resource::<DimensionGenerators>()
        .zip(host.get_resource::<DimensionList>())
        .and_then(|(generators, list)| generators.get(list, build.dimension))
        .and_then(|generator| match generator {
            ChunkGenerator::Noise(generator) => Some(Arc::new(generator.biome_source.clone())),
            _ => None,
        });
    let router = &dimension_router.router;
    let blocks = sub_app.world().resource::<Blocks>().0.clone();
    let current = sub_app.world().resource::<CurrentRegistries>();
    let mut diagnostics = Diagnostics::default();
    let surface_ids = SurfaceIds::resolve(current, &mut diagnostics);
    let fill_ids = FillIds::resolve(current, &mut diagnostics);
    let preset_tables = match biome_source.as_deref() {
        Some(BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(_))) => {
            PresetBiomeTables::resolve(current, &mut diagnostics)
        }
        _ => Some(PresetBiomeTables::default()),
    };
    let (Some(surface_ids), Some(fill_ids), Some(preset_tables)) =
        (surface_ids, fill_ids, preset_tables)
    else {
        error!(%dimension, %diagnostics, "the registries lack a name worldgen needs; it will generate nothing");
        return;
    };
    let features = host
        .get_resource::<DimensionFeaturePrograms>()
        .and_then(|programs| programs.0.get(dimension))
        .map(Arc::clone);
    if features.is_none() {
        warn!(%dimension, "no feature program for this dimension; it will place no features");
    }
    let saved = host.get_resource::<WorldSave>().and_then(|save| {
        let registries = Registries::clone(sub_app.world().resource::<CurrentRegistries>());
        SavedColumns::open(&save.0, build.dimension, registries)
    });
    let fill = FillContext::build(
        Arc::clone(router),
        Some(Arc::clone(&dimension_router.material)),
        blocks,
        dimension_y_sections(
            router,
            build.type_config.min_y,
            build.type_config.section_count,
        ),
        biome_source,
        &preset_tables,
        host.get_resource::<BlockHeightmapPredicates>()
            .map(|predicates| predicates.0.clone()),
        saved,
        host.get_resource::<DimensionCarverBiomes>()
            .and_then(|carvers| carvers.0.get(dimension))
            .map(Arc::clone),
        Arc::new(surface_ids),
        &fill_ids,
        features,
        host.get_resource::<DimensionStructures>()
            .and_then(|structures| structures.0.get(dimension))
            .cloned(),
    );
    sub_app.insert_resource(fill);
}
