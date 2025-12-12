use crate::world::generate::generate_chunk;
use crate::world::palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedPreUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, ResMut};
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_engine::world::chunk::{ChunkPos, ChunkStatus};
use rustc_hash::FxHashMap;
use std::sync::OnceLock;

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        CHUNK_TASK_POOL.get_or_init(|| {
            TaskPoolBuilder::new()
                .thread_name("ChunkGen".to_string())
                .num_threads(4)
                .build()
        });
        app.insert_resource(LoadingChunks::default());
        app.add_systems(
            FixedPreUpdate,
            (load_chunks, process_generated_chunk).chain(),
        );
    }
}

static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

#[derive(Resource, Default, Debug)]
struct LoadingChunks(FxHashMap<ChunkPos, Task<ChunkLoadingTask>>);

struct ChunkLoadingTask {
    chunk: Entity,
    pos: ChunkPos,
    blocks: BlockPalette,
    biomes: BiomePalette,
}

fn load_chunks(
    mut query: Query<(Entity, &mut ChunkStatus, &ChunkPos)>,
    mut loading_chunks: ResMut<LoadingChunks>,
) {
    let task_pool = CHUNK_TASK_POOL.get().unwrap();
    query.iter_mut().for_each(|(e, mut status, pos)| {
        if *status != ChunkStatus::Loading {
            return;
        }
        *status = ChunkStatus::Generating;
        let pos = *pos;
        let task = task_pool.spawn(async move {
            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();
            generate_chunk(pos, &mut blocks, &mut biomes);
            ChunkLoadingTask {
                chunk: e,
                pos,
                blocks,
                biomes,
            }
        });
        loading_chunks.0.insert(pos, task);
    })
}

fn process_generated_chunk(
    mut loading_chunks: ResMut<LoadingChunks>,
    mut query: Query<(&mut ChunkStatus)>,
    mut commands: Commands,
) {
    loading_chunks.0.retain(|pos, task| {
        let res = block_on(future::poll_once(task));
        let retain = res.is_none();
        if let Some(loaded_chunk) = res {
            let chunk = loaded_chunk.chunk;
            let Ok((mut status)) = query.get_mut(chunk) else {
                return false;
            };
            *status = ChunkStatus::Loaded;
            commands
                .entity(chunk)
                .insert((loaded_chunk.blocks, loaded_chunk.biomes));
        }
        retain
    })
}
