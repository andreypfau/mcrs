use crate::world::generate::generate_column;
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
use mcrs_engine::world::chunk::{ChunkGenerating, ChunkLoaded, ChunkLoading, ChunkPos};
use mcrs_minecraft_worldgen::bevy::{NoiseGeneratorSettingsPlugin, OverworldNoiseRouter};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
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

/// Token for cooperative cancellation of chunk generation tasks.
///
/// The token is cloned and passed to worker tasks. When `cancel()` is called,
/// tasks check `is_cancelled()` between section generations and can exit early.
#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Create a new uncancelled token.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Signal cancellation to all clones of this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Check if cancellation has been signaled.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

struct ChunkColumnResult {
    sections: Vec<(Entity, ChunkPos, BlockPalette, BiomePalette)>,
}

#[derive(Resource, Default)]
struct LoadingChunks {
    tasks: Vec<Task<ChunkColumnResult>>,
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
    query: Query<(Entity, &ChunkPos), With<ChunkLoading>>,
    mut loading_chunks: ResMut<LoadingChunks>,
    overworld_noise_router: Res<OverworldNoiseRouter>,
    players: Query<&Transform, With<Player>>,
) {
    if query.is_empty() {
        return;
    }

    let task_pool = CHUNK_TASK_POOL.get().unwrap();

    // Group sections by (x, z) column
    let mut columns: HashMap<(i32, i32), Vec<(Entity, ChunkPos)>> = HashMap::new();
    for (entity, pos) in query.iter() {
        commands
            .entity(entity)
            .insert(ChunkGenerating)
            .remove::<ChunkLoading>();
        columns
            .entry((pos.x, pos.z))
            .or_default()
            .push((entity, *pos));
    }

    let mut dispatched = 0usize;

    for ((col_x, col_z), mut sections) in columns {
        // Sort sections by Y so generation proceeds bottom-to-top
        sections.sort_by_key(|(_, pos)| pos.y);

        let router = overworld_noise_router.0.clone();
        let task = task_pool.spawn(async move {
            let router = router.as_ref();
            let _span = tracing::info_span!("ChunkColumnGen").entered();

            let y_sections: Vec<i32> = sections.iter().map(|(_, pos)| pos.y).collect();
            let results = generate_column(col_x, col_z, &y_sections, router);

            let column_sections = sections
                .into_iter()
                .zip(results)
                .map(|((entity, pos), (blocks, biomes))| (entity, pos, blocks, biomes))
                .collect();

            ChunkColumnResult {
                sections: column_sections,
            }
        });

        loading_chunks.tasks.push(task);
        dispatched += 1;
    }

    if dispatched > 0 {
        info!("Dispatched generation tasks for {} columns", dispatched);
    }
}

fn process_generated_chunk(mut loading_chunks: ResMut<LoadingChunks>, mut commands: Commands) {
    loading_chunks.tasks.retain_mut(|task| {
        let res = block_on(future::poll_once(task));
        if let Some(column_result) = res {
            for (entity, _pos, blocks, biomes) in column_result.sections {
                commands
                    .entity(entity)
                    .insert((ChunkLoaded, blocks, biomes))
                    .remove::<ChunkGenerating>();
            }
            false
        } else {
            true
        }
    });
}
