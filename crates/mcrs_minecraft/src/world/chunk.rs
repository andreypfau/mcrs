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
                load_chunks.run_if(resource_exists::<OverworldNoiseRouter>),
                process_generated_chunk,
            ),
        );
    }
}

static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

#[derive(Resource, Default, Debug)]
struct LoadingChunks(Vec<Task<ChunkLoadingTask>>);

struct ChunkLoadingTask {
    chunk: Entity,
    pos: ChunkPos,
    blocks: BlockPalette,
    biomes: BiomePalette,
}

fn load_chunks(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ChunkStatus, &ChunkPos), With<ChunkLoading>>,
    mut loading_chunks: ResMut<LoadingChunks>,
    overworld_noise_router: Res<OverworldNoiseRouter>,
) {
    const MAX_CHUNKS_PER_TICK: usize = 1024;

    if query.is_empty() {
        return;
    }
    let task_pool = CHUNK_TASK_POOL.get().unwrap();

    let mut tmp = Vec::new();

    for (e, mut status, pos) in query.iter_mut() {
        if loading_chunks.0.len() >= MAX_CHUNKS_PER_TICK {
            break;
        }

        // info!("Loading chunk at {:?}", pos);

        *status = ChunkStatus::Generating;
        commands
            .entity(e)
            .insert(ChunkGenerating)
            .remove::<ChunkLoading>();

        let pos = *pos;
        tmp.push(pos);
        let router = overworld_noise_router.0.clone();
        let task = task_pool.spawn(async move {
            let _span = tracing::info_span!("ChunkGen", pos = pos.to_string().as_str()).entered();
            // let mut router = router.as_ref().clone();
            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();
            if pos.x >= 0 && pos.x < 3 && pos.z >= 0 && pos.z < 3 {
                // generate_noise(pos, &mut blocks, &mut biomes, &mut router);
                generate_chunk(pos, &mut blocks, &mut biomes);
            } else {
                generate_chunk(pos, &mut blocks, &mut biomes);
            }
            ChunkLoadingTask {
                chunk: e,
                pos,
                blocks,
                biomes,
            }
        });
        loading_chunks.0.push(task);
    }

    if !tmp.is_empty() {
        info!("Start loading {} chunks", tmp.len());
    }

    tmp.sort_by_key(|pos| (pos.x.abs() + pos.z.abs(), pos.x, pos.z, pos.y));

    tmp.into_iter().for_each(|pos| {
        // info!("Started loading chunk at {:?}", pos);
    });
}

fn process_generated_chunk(mut loading_chunks: ResMut<LoadingChunks>, mut commands: Commands) {
    loading_chunks.0.retain_mut(|task| {
        let res = block_on(future::poll_once(task));
        let retain = res.is_none();
        if let Some(loaded_chunk) = res {
            let chunk = loaded_chunk.chunk;
            // info!("Loaded chunk at {:?}", loaded_chunk.pos);
            commands
                .entity(chunk)
                .insert(ChunkLoaded)
                .remove::<ChunkGenerating>();
            commands
                .entity(chunk)
                .insert((loaded_chunk.blocks, loaded_chunk.biomes));
        }
        retain
    })
}
