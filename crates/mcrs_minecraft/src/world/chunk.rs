use crate::world::generate::{generate_chunk, generate_noise};
use crate::world::palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedPreUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, Resource, With, resource_exists};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Res, ResMut};
use bevy_math::IVec3;
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::chunk::{
    ChunkGenerating, ChunkLoaded, ChunkLoading, ChunkPos, ChunkStatus,
};
use mcrs_minecraft_worldgen::bevy::{NoiseGeneratorSettingsPlugin, OverworldNoiseRouter};
use std::sync::OnceLock;
use tracing::info;

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NoiseGeneratorSettingsPlugin);
        CHUNK_TASK_POOL.get_or_init(|| {
            TaskPoolBuilder::new()
                .thread_name("ChunkGen".to_string())
                .num_threads(4)
                .build()
        });
        app.insert_resource(LoadingChunks::default());
        app.add_systems(
            FixedPreUpdate,
            (
                process_generated_chunk,
                load_chunks.run_if(resource_exists::<OverworldNoiseRouter>),
            )
                .chain(),
        );
    }
}

static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

struct ChunkLoadingTask {
    chunk: Entity,
    pos: ChunkPos,
    blocks: BlockPalette,
    biomes: BiomePalette,
}

#[derive(Resource, Default)]
struct LoadingChunks {
    tasks: Vec<Task<ChunkLoadingTask>>,
}

/// Squared XZ (column) distance from a chunk to the nearest player.
fn min_column_distance(pos: &ChunkPos, players: &[IVec3]) -> i64 {
    if players.is_empty() {
        return 0;
    }
    players
        .iter()
        .map(|p| {
            let dx = (pos.x - p.x) as i64;
            let dz = (pos.z - p.z) as i64;
            dx * dx + dz * dz
        })
        .min()
        .unwrap_or(0)
}

/// Absolute Y distance from a chunk to the nearest player's Y.
fn min_y_distance(pos: &ChunkPos, players: &[IVec3]) -> i32 {
    if players.is_empty() {
        return 0;
    }
    players
        .iter()
        .map(|p| (pos.y - p.y).abs())
        .min()
        .unwrap_or(0)
}

fn load_chunks(
    mut commands: Commands,
    mut query: Query<(Entity, &ChunkPos), With<ChunkLoading>>,
    mut loading_chunks: ResMut<LoadingChunks>,
    overworld_noise_router: Res<OverworldNoiseRouter>,
    players: Query<&Transform, With<Player>>,
) {
    if query.is_empty() {
        return;
    }

    let task_pool = CHUNK_TASK_POOL.get().unwrap();
    let mut dispatched = 0usize;

    for (e, pos) in query.iter() {
        let pos = *pos;
        // info!("Loading chunk at {:?}", pos);

        commands
            .entity(e)
            .insert(ChunkGenerating)
            .remove::<ChunkLoading>();

        let router = overworld_noise_router.0.clone();
        let task = task_pool.spawn(async move {
            let router = router.as_ref();
            let _span = tracing::info_span!("ChunkGen").entered();
            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();
            if pos.y > 2 && pos.y <= 8 && pos.x >= 0 && pos.x < 2 && pos.z >= 0 && pos.z < 2 {
                let _span = tracing::info_span!("ChunkGen::generate_noise").entered();
                generate_noise(pos, &mut blocks, &mut biomes, &router);
            } else {
                let _span = tracing::info_span!("ChunkGen::generate_chunk").entered();
                generate_chunk(pos, &mut blocks, &mut biomes);
            }
            ChunkLoadingTask {
                chunk: e,
                pos,
                blocks,
                biomes,
            }
        });

        loading_chunks.tasks.push(task);
        dispatched += 1;
    }

    if dispatched > 0 {
        info!("Dispatched generation tasks for {} chunks", dispatched);
    }
}

fn process_generated_chunk(mut loading_chunks: ResMut<LoadingChunks>, mut commands: Commands) {
    loading_chunks.tasks.retain_mut(|task| {
        let res = block_on(future::poll_once(task));
        if let Some(loaded_chunk) = res {
            commands
                .entity(loaded_chunk.chunk)
                .insert((ChunkLoaded, loaded_chunk.blocks, loaded_chunk.biomes))
                .remove::<ChunkGenerating>();
            false
        } else {
            true
        }
    });
}
