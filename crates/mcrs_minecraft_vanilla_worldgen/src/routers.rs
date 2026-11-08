use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf, rl};
use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_generator::routers::{build_router, refuse_misplaced_beardifier};
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use tracing::{error, info};

use crate::structures::DimensionStructures;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

const FLAT_NOISE_SETTINGS: ResourceLocation<'static> = rl!("minecraft:overworld");

/// Every dimension's compiled router, keyed by the id the world preset gave it.
///
/// A router is immutable once compiled, so the host builds each one and hands
/// the dimension's sub-app an `Arc` of it. Compiling inside the sub-app instead
/// would make every dimension re-read the worldgen corpus off disk.
#[derive(Resource, Default, Clone)]
pub struct DimensionRouters(pub BTreeMap<ResourceLocationBuf, DimensionRouter>);

/// A router and the material rules compiled into its graph.
#[derive(Clone)]
pub struct DimensionRouter {
    pub router: Arc<NoiseRouter>,
    pub material: Arc<MaterialProgram>,
}

/// Compiles one router per noise dimension of the baked list.
///
/// Runs at `OnEnter(AppState::Playing)`, before the spawn requests are
/// enqueued, from the loaded registry set, so no sub-app exists yet to miss a
/// router.
pub(crate) fn build_dimension_routers(
    mut commands: Commands,
    dimensions: Res<DimensionGenerators>,
    list: Res<DimensionList>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    structures: Option<Res<DimensionStructures>>,
) {
    let Some(noise_settings) = registries.registry::<NoiseGeneratorSettings>() else {
        error!(
            "the registries declare no noise settings registry; no dimension gets a noise router"
        );
        return;
    };

    let mut routers = DimensionRouters::default();
    for (dimension, generator) in dimensions.iter(&list) {
        // A flat dimension fills no density graph, but its columns take their height and seed
        // from a router, so it gets the overworld's. A debug generator is left out.
        let settings_id = match generator {
            ChunkGenerator::Noise(generator) => generator.settings,
            ChunkGenerator::Flat(_) => match noise_settings.by_name(FLAT_NOISE_SETTINGS) {
                Some(id) => id,
                None => {
                    error!(dimension = %dimension.location(), "no overworld noise settings for a flat dimension");
                    continue;
                }
            },
            ChunkGenerator::Debug => continue,
        };
        let dimension = dimension.location();
        let (Some(settings), Some(settings_name)) = (
            noise_settings.get(settings_id),
            noise_settings.name(settings_id),
        ) else {
            error!(%dimension, id = ?settings_id, "the dimension names noise settings the registry does not number");
            continue;
        };
        if let Some(tables) = structures
            .as_ref()
            .and_then(|tables| tables.0.get(dimension))
        {
            refuse_misplaced_beardifier(&registries, dimension, settings_name, settings, tables);
        }
        match build_router(&registries, settings, seed.0, &blocks) {
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
