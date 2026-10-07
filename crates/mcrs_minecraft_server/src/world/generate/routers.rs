use crate::loaded::Loaded;
use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_generator::routers::{build_router, refuse_misplaced_beardifier};
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use tracing::{error, info};

use crate::world::generate::structures::DimensionStructures;
use crate::world_options::{DimensionList, WorldSeed};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

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
/// enqueued, from the loaded registry set, so no sub-app exists yet to miss a
/// router.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dimension_routers(
    mut commands: Commands,
    dimensions: Res<DimensionList>,
    seed: Res<WorldSeed>,
    blocks: Res<Blocks>,
    registries: Res<RegistrySet>,
    structures: Res<DimensionStructures>,
) {
    let biome_tags = registries.loaded_tags::<Biome>();
    let noise_settings = registries.loaded_registry::<NoiseGeneratorSettings>();
    let settings_values =
        registries.loaded_entries::<NoiseGeneratorSettings, NoiseGeneratorSettings>();

    let mut routers = DimensionRouters::default();
    for (dimension, entry) in dimensions.iter() {
        // A flat or debug generator drives no density graph, so the dimension
        // is left out rather than refused.
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let dimension = dimension.location();
        let Some(settings) = settings_values.get(generator.settings) else {
            error!(%dimension, id = ?generator.settings, "the dimension names noise settings the registry does not number");
            continue;
        };
        let settings_name = noise_settings
            .name(generator.settings)
            .expect("an id of the registry has a name");
        if let Some(tables) = structures.0.get(dimension) {
            refuse_misplaced_beardifier(&registries, dimension, settings_name, settings, tables);
        }
        match build_router(&registries, settings, seed.0, &blocks, &biome_tags) {
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
