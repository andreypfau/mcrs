use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen::bevy::WorldgenAssets;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, lookup_id};
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_generator::routers::{build_router, refuse_misplaced_beardifier};
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use tracing::{error, info};

use crate::world::generate::structures::DimensionStructures;
use crate::world_options::{DimensionList, WorldSeed};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

/// Every dimension's biome source, keyed by the id the world preset gave it.
///
/// The source decides which biome a column reports and therefore which surface
/// rules and carvers it runs, so it belongs to the same dimension as the router
/// it is sampled beside.
#[derive(Resource, Default, Clone)]
pub struct DimensionBiomeSources(
    pub BTreeMap<ResourceLocation, Arc<mcrs_minecraft_biome::source::BiomeSource>>,
);

/// Every dimension's compiled router, keyed by the id the world preset gave it.
///
/// A router is immutable once compiled, so the host builds each one and hands
/// the dimension's sub-app an `Arc` of it. Compiling inside the sub-app instead
/// would make every dimension re-read the worldgen corpus off disk.
#[derive(Resource, Default, Clone)]
pub struct DimensionRouters(pub BTreeMap<ResourceLocation, DimensionRouter>);

/// A router and the material rules compiled into its graph.
#[derive(Clone)]
pub struct DimensionRouter {
    pub router: Arc<NoiseRouter>,
    pub material: Arc<MaterialProgram>,
}

/// Compiles one router per noise dimension of the baked list.
///
/// Runs at `OnEnter(AppState::Playing)`, before the spawn requests are
/// enqueued: `WorldgenFreeze` waits on the whole recursive dependency closure
/// of the dimensions' noise settings, so every asset a router needs has landed by
/// here, and no sub-app exists yet to miss one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dimension_routers(
    mut commands: Commands,
    dimensions: Res<DimensionList>,
    seed: Res<WorldSeed>,
    tables: Res<WorldgenTables>,
    assets: WorldgenAssets,
    blocks: Res<Blocks>,
    registries: Res<RegistrySet>,
    structures: Option<Res<DimensionStructures>>,
) {
    let biome_tags = registries
        .tags::<Biome>()
        .expect("the data pack loader builds the biome tags");
    let noise_settings = registries
        .registry::<NoiseGeneratorSettings>()
        .expect("the data pack declares minecraft:worldgen/noise_settings");

    let mut routers = DimensionRouters::default();
    for (dimension, entry) in dimensions.iter() {
        // A flat or debug generator drives no density graph, so the dimension
        // is left out rather than refused.
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let dimension = dimension.location();
        let settings = match lookup_id(&noise_settings, &tables.noise_settings, generator.settings)
        {
            Ok(settings) => settings,
            Err(error) => {
                error!(%dimension, %error, "the noise settings of this dimension are unavailable");
                continue;
            }
        };
        let settings_name = noise_settings
            .name(generator.settings)
            .expect("an id of the registry has a name");
        if let Some(tables) = structures.as_ref().and_then(|s| s.0.get(dimension)) {
            refuse_misplaced_beardifier(dimension, settings_name, settings, &assets, tables);
        }
        match build_router(settings, &assets, seed.0, &blocks, &biome_tags) {
            Ok((router, material)) => {
                info!(%dimension, seed = seed.0, "compiled the dimension noise router");
                routers.0.insert(
                    dimension.clone(),
                    DimensionRouter {
                        router: Arc::new(router),
                        material: Arc::new(material),
                    },
                );
            }
            Err(error) => error!(
                %dimension, %error,
                "the noise settings did not compile; this dimension will generate nothing"
            ),
        }
    }
    commands.insert_resource(routers);
}
