use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_asset::Assets;
use bevy_ecs::prelude::{Commands, Res, Resource};
use mcrs_minecraft_core::{RegistrySnapshot, ResourceLocation};
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::block::definition::Blocks;
use mcrs_minecraft_world::dimension::level_stem::DimensionDefinition;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen::bevy::{
    NoiseGeneratorSettingsAsset, WorldgenAssets, build_dimension_router,
};
use mcrs_minecraft_worldgen::router::NoiseRouter;
use tracing::{error, info};

use crate::configuration::{LoadedWorldPreset, WorldSeed};
use crate::world::chunk::try_resolve_state;

/// Every dimension's compiled router, keyed by the id the world preset gave it.
///
/// A router is immutable once compiled, so the host builds each one and hands
/// the dimension's sub-app an `Arc` of it. Compiling inside the sub-app instead
/// would make every dimension re-read the worldgen corpus off disk.
#[derive(Resource, Default, Clone)]
pub struct DimensionRouters(pub BTreeMap<ResourceLocation, Arc<NoiseRouter>>);

/// Compiles one router per noise dimension of the loaded preset.
///
/// Runs at `OnEnter(AppState::Playing)`, before the spawn requests are
/// enqueued: `WorldgenFreeze` waits on the preset's whole recursive dependency
/// closure, so every asset a router needs has landed by here, and no sub-app
/// exists yet to miss one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dimension_routers(
    mut commands: Commands,
    preset: Res<LoadedWorldPreset>,
    seed: Res<WorldSeed>,
    definitions: Res<Assets<DimensionDefinition>>,
    settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
    assets: WorldgenAssets,
    blocks: Res<Blocks>,
    biome_registry: Res<RegistrySnapshot<Biome>>,
) {
    // The material rules take their biome ids from this snapshot, because it is
    // what `MultiNoiseBiomeTable` fills the column's biome grid with.
    let biome_ids: BTreeMap<ResourceLocation, u32> = biome_registry
        .iter()
        .map(|(network_id, entry)| (entry.location.clone(), network_id))
        .collect();
    let block = |state: &_| try_resolve_state(&blocks, state).map(Into::into);
    let biome = |id: &ResourceLocation| biome_ids.get(id).copied();

    let mut routers = DimensionRouters::default();
    for (dimension, handle) in &preset.dimensions {
        let Some(definition) = definitions.get(handle) else {
            error!(%dimension, "the dimension definition did not load");
            continue;
        };
        // A flat or debug generator drives no density graph, so the dimension
        // is left out rather than refused.
        let ChunkGenerator::Noise(generator) = &definition.generator else {
            continue;
        };
        let Some(asset) = settings.get(&generator.settings) else {
            error!(%dimension, "the noise settings named by this dimension did not load");
            continue;
        };
        match build_dimension_router(asset, &assets, seed.0, &block, &biome) {
            Ok(router) => {
                info!(%dimension, seed = seed.0, "compiled the dimension noise router");
                routers.0.insert(dimension.clone(), Arc::new(router));
            }
            Err(error) => error!(
                %dimension, %error,
                "the noise settings did not compile; this dimension will generate nothing"
            ),
        }
    }
    commands.insert_resource(routers);
}
