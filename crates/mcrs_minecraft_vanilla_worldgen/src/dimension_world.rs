use std::sync::Arc;

use bevy_app::{FixedUpdate, SubApp};
use bevy_ecs::prelude::Res;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use mcrs_engine_cell::CellIndex;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::column_store::{ColumnStoreLink, StoredColumnLandSet};
use mcrs_minecraft_engine::heightmap::BlockHeightmapPredicates;
use mcrs_minecraft_engine::stored_column::PendingColumnFacts;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_registries::{CurrentRegistries, Diagnostics};
use mcrs_minecraft_worldgen_generator::SurfaceIds;
use mcrs_minecraft_worldgen_generator::ids::FillIds;
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use mcrs_minecraft_worldgen_generator::stages::{
    FillContext, GeneratorSetup, dimension_y_sections, flat_y_sections,
};
use tracing::{error, warn};

use crate::features::DimensionFeaturePrograms;
use crate::flat_world::{flat_biome, flat_layers};
use crate::modern_carvers::DimensionCarverBiomes;
use crate::routers::DimensionRouters;
use crate::saved_monuments::rebuild_saved_monument_rooms;
use crate::structures::DimensionStructures;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;

pub(crate) fn build_dimension_world(
    host: &World,
    build: &DimensionBuild<'_>,
    sub_app: &mut SubApp,
) {
    // The host generates the columns of a world from the context of its first cell, so the
    // other cells need none.
    let first_cell = sub_app
        .world()
        .get_resource::<OwnedColumns>()
        .is_none_or(|owned| owned.index() == CellIndex(0));
    if first_cell {
        insert_fill_context(host, build, sub_app);
    }
    // The world generates its columns in the host, and lands each one as an answer of the data
    // layer.
    if let Some(mut store) = sub_app.world_mut().get_resource_mut::<ColumnStoreLink>() {
        store.generate_through_world();
    }
    sub_app.add_systems(
        FixedUpdate,
        rebuild_saved_monument_rooms
            .in_set(TickSet::DomainInput)
            .after(StoredColumnLandSet)
            .run_if(|landed: Option<Res<PendingColumnFacts>>| {
                landed.is_some_and(|landed| !landed.0.is_empty())
            }),
    );
    sub_app.insert_resource(
        host.get_resource::<DimensionStructures>()
            .cloned()
            .unwrap_or_default(),
    );
}

fn insert_fill_context(host: &World, build: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    let dimension = build.dimension.location();
    let generator = host
        .get_resource::<DimensionGenerators>()
        .zip(host.get_resource::<DimensionList>())
        .and_then(|(generators, list)| generators.get(list, build.dimension));
    let blocks = sub_app.world().resource::<Blocks>().0.clone();
    let current = sub_app.world().resource::<CurrentRegistries>();
    let (setup, biome_source, y_sections) = match generator {
        Some(ChunkGenerator::Noise(generator)) => {
            let Some(dimension_router) = host
                .get_resource::<DimensionRouters>()
                .and_then(|routers| routers.0.get(dimension))
            else {
                warn!(%dimension, "no noise router for this dimension; it will generate nothing");
                return;
            };
            let router = &dimension_router.router;
            let y_sections = dimension_y_sections(
                router,
                build.type_config.min_y,
                build.type_config.section_count,
            );
            (
                GeneratorSetup::Noise {
                    router: Arc::clone(router),
                    material: Some(Arc::clone(&dimension_router.material)),
                },
                Some(Arc::new(generator.biome_source.clone())),
                y_sections,
            )
        }
        Some(ChunkGenerator::Flat(generator)) => (
            GeneratorSetup::Flat(Arc::new(flat_layers(generator, &blocks))),
            flat_biome(generator, current).map(|biome| Arc::new(BiomeSource::Fixed { biome })),
            flat_y_sections(build.type_config.min_y, build.type_config.section_count),
        ),
        _ => {
            warn!(%dimension, "no noise router for this dimension; it will generate nothing");
            return;
        }
    };
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
    let Some(predicates) = host
        .get_resource::<BlockHeightmapPredicates>()
        .map(|predicates| predicates.0.clone())
    else {
        error!(%dimension, "no heightmap predicates for this dimension; it will generate nothing");
        return;
    };
    let features = match setup {
        GeneratorSetup::Noise { .. } => {
            let features = host
                .get_resource::<DimensionFeaturePrograms>()
                .and_then(|programs| programs.0.get(dimension))
                .map(Arc::clone);
            if features.is_none() {
                warn!(%dimension, "no feature program for this dimension; it will place no features");
            }
            features
        }
        GeneratorSetup::Flat(_) => None,
    };
    let fill = FillContext::build(
        setup,
        blocks,
        y_sections,
        biome_source,
        &preset_tables,
        predicates,
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
