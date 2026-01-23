use crate::world::generate::generate_chunk;
use crate::world::palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedPreUpdate, Plugin, Startup};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, Resource};
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Res, ResMut};
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_engine::world::chunk::{ChunkPos, ChunkStatus};
use mcrs_minecraft_worldgen::bevy::{NoiseGeneratorSettingsAsset, NoiseGeneratorSettingsPlugin};
use rustc_hash::FxHashMap;
use std::sync::OnceLock;

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
    let _span = tracing::info_span!("load_chunks get pool").entered();
    let task_pool = CHUNK_TASK_POOL.get().unwrap();
    drop(_span);
    let _span = tracing::info_span!("load_chunks iterate chunks").entered();
    query.iter_mut().for_each(|(e, mut status, pos)| {
        let _1 = tracing::info_span!("load_chunks process chunk").entered();
        if *status != ChunkStatus::Loading {
            return;
        }
        *status = ChunkStatus::Generating;
        let pos = *pos;
        let _2 = tracing::info_span!("load_chunks spawn task").entered();
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
        let _3 = tracing::info_span!("load_chunks insert task").entered();
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
