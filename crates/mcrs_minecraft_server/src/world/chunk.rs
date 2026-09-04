use crate::world::format::anvil::{SavedColumns, SectionData, column_sections};
use crate::world::generate::{
    BetaCaveBlockIds, BetaOreBlockIds, apply_beta_caves, apply_beta_ores, apply_beta_surface,
    generate_column,
};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, Resource, With, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Commands, Local, Res, ResMut};
use bevy_math::IVec3;
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::source::BiomeSource;
use mcrs_minecraft_world::block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_world::worldgen::beta_biome::{ActiveBiomeSource, BetaBiomeSourcePlugin};
use mcrs_minecraft_worldgen::bevy::{
    BuildNoiseRouter, NoiseGeneratorSettingsAsset, NoiseGeneratorSettingsPlugin,
    OverworldNoiseRouter, WorldGenConfig,
};
use mcrs_minecraft_worldgen::proto::BlockState as ProtoBlockState;
use mcrs_voxel_light::storage::LightStorage;
use mcrs_voxel_light::{BlockLight, SkyLight};
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_world::entity::physics::Transform;
use mcrs_voxel_world::entity::player::Player;
use mcrs_voxel_world::entity::player::chunk_view::PlayerChunkObserver;
use mcrs_voxel_world::world::lifecycle::markers::ChunkGenerating;
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoaded;
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoading;
use mcrs_voxel_world::world::lifecycle::markers::ChunkUnloading;
use mcrs_voxel_world::world::lifecycle::trace as column_trace;
use mcrs_voxel_world::world::lifecycle::trace::ColumnStage;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};
use tracing::{error, info, info_span, trace};

/// The noise settings state which block fills the terrain and which fluid fills
/// the sea (`minecraft:stone` and `minecraft:water` for the overworld), so the
/// ids the generator writes come from that asset resolved against the corpus.
fn resolve_worldgen_default_states(
    mut messages: bevy_ecs::message::MessageReader<
        bevy_asset::AssetEvent<NoiseGeneratorSettingsAsset>,
    >,
    settings: Res<bevy_asset::Assets<NoiseGeneratorSettingsAsset>>,
    blocks: Res<Blocks>,
    mut config: ResMut<WorldGenConfig>,
) {
    for message in messages.read() {
        let bevy_asset::AssetEvent::LoadedWithDependencies { id } = message else {
            continue;
        };
        let Some(asset) = settings.get(*id) else {
            continue;
        };
        let default_block = resolve_state(&blocks, &asset.settings.default_block);
        let default_fluid = resolve_state(&blocks, &asset.settings.default_fluid);
        config.default_block_state_id = Some(default_block.into());
        config.default_fluid_state_id = Some(default_fluid.into());
        trace!(
            default_block = default_block.0,
            default_fluid = default_fluid.0,
            "resolved the noise settings default states"
        );
    }
}

fn resolve_state(
    blocks: &BlockDefinitions,
    state: &ProtoBlockState,
) -> mcrs_minecraft_protocol::BlockStateId {
    let name = state.name.as_str();
    let block = blocks
        .block(name)
        .unwrap_or_else(|| panic!("the noise settings name `{name}`, which no block declares"));
    let mut id = block.default_state_id;
    for (property, value) in state.properties.iter().flatten() {
        id = block
            .with_text(id, property, value)
            .unwrap_or_else(|| panic!("`{name}` declares no `{property}` that reads `{value}`"));
    }
    id
}

/// Ordering anchor for the worldgen ingest path. The lighting plugin chains
/// its enqueue set after `WorldgenIngestSet::ProcessCompletedColumns` so the
/// `Added<ChunkLoaded>` filters in the enqueue systems observe the newly-
/// generated sections within the same tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorldgenIngestSet {
    ProcessCompletedColumns,
}

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NoiseGeneratorSettingsPlugin);
        app.add_plugins(BetaBiomeSourcePlugin);
        if !app.world().contains_resource::<WorldGenConfig>() {
            app.insert_resource(WorldGenConfig::from_env());
        }
        app.add_systems(
            bevy_app::Update,
            resolve_worldgen_default_states.before(BuildNoiseRouter),
        );
        let scheduler = ColumnScheduler::default();
        CHUNK_TASK_POOL.get_or_init(|| {
            TaskPoolBuilder::new()
                .thread_name("ChunkGen".to_string())
                .num_threads(scheduler.config.num_threads)
                .build()
        });
        app.insert_resource(scheduler);
        // The chunks a view ticketed this tick are spawned this tick and dispatched right
        // after; what lands is drained by the dimension's `ColumnDrain`, at the tick's end and
        // between ticks.
        app.add_systems(
            FixedUpdate,
            (
                enqueue_pending_columns,
                cancel_stale_columns,
                reprioritize_columns,
                dispatch_column_generation.run_if(resource_exists::<OverworldNoiseRouter>),
            )
                .chain()
                .after(mcrs_voxel_world::world::lifecycle::ticket::ChunkSpawnSet),
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

/// Sort key for the priority queue. Lower distance_sq values are dequeued first.
/// The column position (col_x, col_z) serves as a tiebreaker for determinism.
#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Debug)]
pub struct ColumnKey {
    /// Squared XZ distance to the nearest player. Primary sort key.
    pub distance_sq: i32,
    /// Column position. Tiebreaker for deterministic ordering.
    pub chunk_column_pos: ColumnPos,
}

impl ColumnKey {
    /// Create a new ColumnKey with the given distance and position.
    pub fn new(distance_sq: i32, chunk_column_pos: ColumnPos) -> Self {
        Self {
            distance_sq,
            chunk_column_pos,
        }
    }
}

/// A column waiting in the priority queue to be dispatched for generation.
///
/// Contains the list of section entities and their Y positions. Sections are stored
/// as `(Entity, y)` tuples where `y` is the section's Y coordinate.
pub struct PendingColumn {
    /// Section entities with their Y coordinates, to be sorted by Y before dispatch.
    pub sections: Vec<(Entity, i32)>,
    /// When the column joined the queue, so a column that arrives late can say
    /// whether it waited for a worker or for the work itself.
    pub queued: Instant,
}

impl PendingColumn {
    /// Create a new pending column with the given sections.
    pub fn new(sections: Vec<(Entity, i32)>) -> Self {
        Self {
            sections,
            queued: Instant::now(),
        }
    }
}

/// A column currently being generated by a worker task.
///
/// Tracks the column position, section entities, cancellation token, and the async task.
/// When the task completes or is cancelled, the sections are processed accordingly.
pub struct InFlightColumn {
    /// Column position (x, z) in chunk coordinates.
    pub col: ColumnPos,
    /// Section entities with their Y coordinates.
    pub sections: Vec<(Entity, i32)>,
    /// Token to signal cancellation to the worker task.
    pub cancel: CancellationToken,
    /// The async task generating this column.
    pub task: Task<ColumnResult>,
    pub queued: Instant,
    pub dispatched: Instant,
}

impl InFlightColumn {
    /// Create a new in-flight column with the given parameters.
    pub fn new(
        col: ColumnPos,
        sections: Vec<(Entity, i32)>,
        cancel: CancellationToken,
        task: Task<ColumnResult>,
        queued: Instant,
    ) -> Self {
        Self {
            col,
            sections,
            cancel,
            task,
            queued,
            dispatched: Instant::now(),
        }
    }
}

/// Configuration for the chunk column scheduler.
///
/// Controls concurrency limits and dispatch rates for chunk generation tasks.
#[derive(Resource, Clone)]
pub struct SchedulerConfig {
    /// Maximum number of concurrent generation tasks.
    /// Default: `available_parallelism * 2` (or 8 if unavailable).
    pub max_in_flight: usize,
    /// Maximum columns to dispatch per tick.
    /// Default: 32.
    pub max_dispatch_per_tick: usize,
    /// Number of threads in the chunk generation thread pool.
    /// Default: 4.
    pub num_threads: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        Self {
            max_in_flight: parallelism * 8,
            max_dispatch_per_tick: 256,
            num_threads: (parallelism / 2).max(4),
        }
    }
}

/// Priority-based scheduler for chunk column generation.
///
/// Manages the lifecycle of chunk columns from pending to in-flight to completed.
/// Uses a `BTreeMap` priority queue for O(log n) priority-ordered dispatch and efficient
/// removal during cancellation/reprioritization.
///
/// # Structure
/// - `pending`: Priority queue of columns waiting to be dispatched (ordered by `ColumnKey`)
/// - `priority_index`: Reverse index from column position to its current priority key
/// - `in_flight_index`: Set of column positions currently being generated
/// - `in_flight`: Active generation tasks with their cancellation tokens
/// - `config`: Concurrency and dispatch limits
#[derive(Resource)]
pub struct ColumnScheduler {
    /// Priority queue of pending columns. Lower `ColumnKey` values are dispatched first.
    pub pending: BTreeMap<ColumnKey, PendingColumn>,
    /// Reverse index: column position -> current priority key.
    /// Enables O(log n) removal/reprioritization by position.
    pub priority_index: FxHashMap<ColumnPos, ColumnKey>,
    /// Set of column positions with active generation tasks.
    /// Used to avoid duplicate dispatch.
    pub in_flight_index: FxHashSet<ColumnPos>,
    /// Active generation tasks with cancellation tokens.
    pub in_flight: Vec<InFlightColumn>,
    /// Configuration for concurrency limits and dispatch rates.
    pub config: SchedulerConfig,
}

impl Default for ColumnScheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

impl ColumnScheduler {
    /// Create a new scheduler with the given configuration.
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            pending: BTreeMap::new(),
            priority_index: FxHashMap::default(),
            in_flight_index: FxHashSet::default(),
            in_flight: Vec::new(),
            config,
        }
    }

    /// Returns the number of columns waiting to be dispatched.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of columns currently being generated.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Check if a column is pending dispatch.
    pub fn is_pending(&self, col: ColumnPos) -> bool {
        self.priority_index.contains_key(&col)
    }

    /// Check if a column has an active generation task.
    pub fn is_in_flight(&self, col: ColumnPos) -> bool {
        self.in_flight_index.contains(&col)
    }
}

/// Result of a column generation task.
///
/// Contains the list of generated sections for a column. Each section includes
/// the entity, position, and optionally the generated block/biome data.
/// `None` indicates the section was cancelled before generation could complete.
pub struct ColumnResult {
    /// Generated sections. `None` for cancelled sections.
    pub sections: Vec<(Entity, ChunkPos, Option<SectionData>)>,
    /// Where the column came from and how long that took, so a slow column can
    /// name the stage that cost the time.
    pub source: ColumnSource,
    pub work: Duration,
}

#[derive(Copy, Clone, Debug)]
pub enum ColumnSource {
    Saved,
    Generated,
}

impl ColumnSource {
    fn label(self) -> &'static str {
        match self {
            Self::Saved => "save",
            Self::Generated => "worldgen",
        }
    }
}

/// A column slower than this from queue to hand-off gets a line naming the
/// stage that cost the time. `MCRS_SLOW_CHUNK_MS` moves the bar.
static SLOW_COLUMN: LazyLock<Duration> = LazyLock::new(|| {
    let ms = std::env::var("MCRS_SLOW_CHUNK_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(250);
    Duration::from_millis(ms)
});

/// Squared XZ (column) distance from a chunk to the nearest player.
fn min_column_distance(pos: &ColumnPos, players: &[ColumnPos]) -> i32 {
    if players.is_empty() {
        return 0;
    }
    players
        .iter()
        .map(|p| pos.distance_squared(*p))
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

/// Process completed column generation tasks from the scheduler.
///
/// This system polls in-flight tasks and processes their results:
/// - For successfully completed sections: inserts `ChunkLoaded` with block/biome data
/// - For cancelled sections (None): inserts `ChunkUnloading` to trigger cleanup
/// - Removes `ChunkGenerating` marker from all processed sections
/// - Removes completed columns from the `in_flight_index`
///
/// Uses `retain_mut` pattern to efficiently filter completed tasks while iterating.
/// Splits a slow column's latency into the three stages that can own it: the
/// wait for a worker, the read or generation itself, and the wait for this
/// system to notice the task had finished.
/// Nothing computes light any more, so a column the save holds no light for is
/// lit as if it stood under open sky; unlit, it would render black.
fn generated_section((blocks, biomes): (BlockPalette, BiomePalette)) -> SectionData {
    (
        blocks,
        biomes,
        BlockLight::default(),
        SkyLight(LightStorage::Uniform(15)),
    )
}

fn report_column_timing(in_flight: &InFlightColumn, result: &ColumnResult) {
    let total = in_flight.queued.elapsed();
    if total < *SLOW_COLUMN {
        return;
    }
    let ms = |d: Duration| d.as_secs_f32() * 1000.0;
    info!(
        x = in_flight.col.x,
        z = in_flight.col.z,
        source = result.source.label(),
        total_ms = ms(total),
        queued_ms = ms(in_flight.dispatched - in_flight.queued),
        work_ms = ms(result.work),
        drain_ms = ms(in_flight.dispatched.elapsed() - result.work),
        sections = in_flight.sections.len(),
        "slow chunk column"
    );
}

pub(crate) fn process_completed_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    mut commands: Commands,
) {
    // Collect columns to remove from in_flight_index after iteration
    let mut columns_to_remove: Vec<ColumnPos> = Vec::new();

    scheduler.in_flight.retain_mut(|in_flight| {
        let res = block_on(future::poll_once(&mut in_flight.task));
        if let Some(column_result) = res {
            report_column_timing(in_flight, &column_result);
            column_trace::mark(in_flight.col, ColumnStage::Loaded);
            column_trace::set_source(in_flight.col, column_result.source.label());
            // Column generation task completed, process all sections
            for (entity, _pos, result) in column_result.sections {
                match result {
                    Some((blocks, biomes, block_light, sky_light)) => {
                        // Section completed successfully - mark as loaded with data
                        commands
                            .entity(entity)
                            .insert((ChunkLoaded, blocks, biomes, block_light, sky_light))
                            .remove::<ChunkGenerating>();
                    }
                    None => {
                        // Section was cancelled before generation could complete
                        // Mark for unloading so the entity gets cleaned up
                        commands
                            .entity(entity)
                            .insert(ChunkUnloading)
                            .remove::<ChunkGenerating>();
                    }
                }
            }
            // Mark column for removal from in-flight tracking
            columns_to_remove.push(in_flight.col);
            false // Remove from in_flight Vec
        } else {
            true // Keep in in_flight Vec, task still running
        }
    });

    // Remove completed columns from in_flight_index
    for col in columns_to_remove {
        scheduler.in_flight_index.remove(&col);
    }
}

/// Enqueue pending chunk columns from entities with ChunkLoading marker.
///
/// This system:
/// 1. Queries all entities with `ChunkLoading` component (newly requested sections)
/// 2. Groups sections by their (x, z) column position
/// 3. Computes priority based on squared XZ distance to nearest player
/// 4. Inserts columns into the priority queue with their priority key
/// 5. Transitions entities from `ChunkLoading` to `ChunkGenerating` state
///
/// Columns already pending or in-flight are skipped to avoid duplicate work.
/// The `ChunkGenerating` marker is applied immediately to prevent re-discovery
/// on subsequent ticks.
pub(crate) fn enqueue_pending_columns(
    mut commands: Commands,
    mut scheduler: ResMut<ColumnScheduler>,
    loading_query: Query<(Entity, &ChunkPos), With<ChunkLoading>>,
    players: Query<&Transform, With<Player>>,
) {
    if loading_query.is_empty() {
        return;
    }

    // Collect player positions for distance calculations
    let player_positions: Vec<ColumnPos> = players
        .iter()
        .map(|t| ColumnPos::from(t.translation))
        .collect();

    // Group sections by (x, z) column without transitioning state yet.
    // State transition is deferred until we know the column can be enqueued
    // or merged into an existing pending column.
    let mut columns: HashMap<ColumnPos, Vec<(Entity, i32)>> = HashMap::new();
    for (entity, pos) in loading_query.iter() {
        trace!(
            "Requested generation for chunk section at ({}, {}, {})",
            pos.x, pos.y, pos.z
        );

        columns
            .entry(ColumnPos::new(pos.x, pos.z))
            .or_default()
            .push((entity, pos.y));
    }

    for (col, sections) in columns {
        if scheduler.is_pending(col) {
            // Merge new sections into the existing pending column so the
            // column is dispatched with ALL its sections in a single batch.
            if let Some(&key) = scheduler.priority_index.get(&col)
                && let Some(pending) = scheduler.pending.get_mut(&key)
            {
                for &(entity, _) in &sections {
                    commands
                        .entity(entity)
                        .insert(ChunkGenerating)
                        .remove::<ChunkLoading>();
                }
                trace!(
                    "Merged {} sections into pending column {:?}",
                    sections.len(),
                    col
                );
                pending.sections.extend(sections);
            }
            continue;
        }

        if scheduler.is_in_flight(col) {
            // Column is currently being generated. Leave these sections in
            // ChunkLoading so they will be picked up on a subsequent tick
            // after the in-flight task completes.
            continue;
        }

        // New column: transition entities and enqueue
        for &(entity, _) in &sections {
            commands
                .entity(entity)
                .insert(ChunkGenerating)
                .remove::<ChunkLoading>();
        }

        column_trace::mark(col, ColumnStage::Queued);
        let distance_sq = min_column_distance(&col, &player_positions);
        let key = ColumnKey::new(distance_sq, col);
        let pending_column = PendingColumn::new(sections);

        trace!(
            "Enqueued chunk column at ({:?}) for generation - {:?}",
            key, pending_column.sections
        );

        scheduler.pending.insert(key, pending_column);
        scheduler.priority_index.insert(col, key);
    }
}

/// Cancel columns that are no longer within any player's view.
///
/// This system handles cancellation for both pending and in-flight columns:
///
/// **Pending columns:**
/// - Removed from the priority queue and column index
/// - All sections transitioned from `ChunkGenerating` to `ChunkUnloading`
///
/// **In-flight columns:**
/// - Cancellation token signaled via `cancel.cancel()`
/// - The worker task will check `is_cancelled()` between sections and exit early
/// - `process_completed_columns` will handle the partial results on the next tick
///
/// A column is considered "stale" if NONE of its sections are visible to ANY player.
/// This is a conservative check - if even one section might be visible, we keep the column.
fn cancel_stale_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    mut commands: Commands,
    players: Query<&PlayerChunkObserver>,
) {
    // Collect all player views for visibility checks
    let player_views: Vec<_> = players
        .iter()
        .filter_map(|observer| observer.last_last_chunk_tracking_view)
        // .map(|view| ColumnPos::from(view.center))
        .collect();

    // If no players have views, don't cancel anything (edge case during startup)
    if player_views.is_empty() {
        return;
    }

    // Cancel stale pending columns
    // Collect keys to remove first to avoid borrowing issues
    let stale_pending: Vec<(ColumnKey, Vec<Entity>)> = scheduler
        .priority_index
        .iter()
        .filter_map(|(col, key)| {
            let pending = scheduler.pending.get(key)?;
            if !player_views.iter().any(|view| {
                let dx = (col.x - view.center.x).abs();
                let dz = (col.z - view.center.z).abs();
                dx <= view.distance as i32 && dz <= view.distance as i32
            }) {
                // Collect section entities for cleanup
                let entities: Vec<Entity> = pending.sections.iter().map(|(e, _)| *e).collect();
                Some((*key, entities))
            } else {
                None
            }
        })
        .collect();

    // Remove stale pending columns and mark sections for unloading.
    for (key, entities) in stale_pending {
        trace!("Canceling stale column {:?}", key);
        column_trace::forget(key.chunk_column_pos);

        scheduler.pending.remove(&key);
        scheduler.priority_index.remove(&key.chunk_column_pos);

        for entity in entities {
            commands
                .entity(entity)
                .insert(ChunkUnloading)
                .remove::<ChunkGenerating>();
        }
    }

    // Cancel stale in-flight columns
    // For in-flight columns, we just signal cancellation - the worker will check the token
    for in_flight in &scheduler.in_flight {
        if !player_views.iter().any(|view| {
            let dx = (in_flight.col.x - view.center.x).abs();
            let dz = (in_flight.col.z - view.center.z).abs();
            dx <= view.distance as i32 && dz <= view.distance as i32
        }) {
            // Signal cancellation to the worker task
            // The task will check is_cancelled() between sections and exit early
            column_trace::forget(in_flight.col);
            in_flight.cancel.cancel();
        }
    }
}

/// Update column priorities when players move.
///
/// This system recalculates the distance-based priority for all pending columns
/// when any player's position changes. Columns that were far away when initially
/// queued may now be closer (and vice versa) as players move around the world.
///
/// # Algorithm
/// 1. Collect current player chunk positions
/// 2. For each pending column, recalculate the squared XZ distance to nearest player
/// 3. If the distance changed, update the column's position in the priority queue:
///    - Remove from `pending` BTreeMap with the old key
///    - Insert into `pending` with the new key
///    - Update `priority_index` to reflect the new key
///
/// # Performance
/// - Run condition: only executes when any player's `Transform` has `Changed`
/// - O(n log n) where n = number of pending columns
/// - Uses batch update to minimize BTreeMap operations
fn reprioritize_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    players: Query<&Transform, With<Player>>,
) {
    // Early exit if no pending columns to reprioritize
    if scheduler.pending.is_empty() {
        return;
    }

    // Collect player positions in chunk coordinates
    let player_positions: Vec<ColumnPos> = players
        .iter()
        .map(|t| ColumnPos::from(t.translation))
        .collect();

    // If no players, nothing to reprioritize against
    if player_positions.is_empty() {
        return;
    }

    // Collect columns that need reprioritization: (col, old_key, new_key)
    let mut updates: Vec<(ColumnKey, ColumnKey)> = Vec::new();

    for (&col, &old_key) in &scheduler.priority_index {
        // Create a ChunkPos for distance calculation (Y doesn't matter for XZ distance)
        let new_distance_sq = min_column_distance(&col, &player_positions);

        // Only update if distance has changed
        if new_distance_sq != old_key.distance_sq {
            let new_key = ColumnKey::new(new_distance_sq, col);
            updates.push((old_key, new_key));
        }
    }

    // Apply updates: remove with old key, insert with new key
    for (old_key, new_key) in updates {
        // Remove from pending with old key
        if let Some(pending_column) = scheduler.pending.remove(&old_key) {
            // Insert with new key
            scheduler.pending.insert(new_key, pending_column);
            // Update the reverse index
            scheduler
                .priority_index
                .insert(new_key.chunk_column_pos, new_key);
        }
    }
}

/// Dispatch pending columns to worker tasks for generation.
///
/// This system pops columns from the priority queue and spawns generation tasks,
/// respecting concurrency limits:
/// - `max_in_flight`: Maximum concurrent generation tasks
/// - `max_dispatch_per_tick`: Maximum columns to dispatch per system run
///
/// # Algorithm
/// 1. Calculate available capacity: `max_in_flight - current_in_flight`
/// 2. Determine dispatch count: `min(available, max_dispatch_per_tick, pending_count)`
/// 3. Pop the N lowest-priority columns from the BTreeMap (closest to players)
/// 4. For each column:
///    - Sort sections by Y (bottom-to-top for cache efficiency)
///    - Create a CancellationToken for cooperative cancellation
///    - Spawn the generation task to the thread pool
///    - Add to `in_flight` Vec and `in_flight_index` set
///
/// # Performance
/// - Uses `pop_first()` for O(log n) priority dequeue from BTreeMap
/// - Bounded dispatch prevents task queue explosion during player teleports
pub(crate) fn dispatch_column_generation(
    mut scheduler: ResMut<ColumnScheduler>,
    overworld_noise_router: Res<OverworldNoiseRouter>,
    blocks: Res<Blocks>,
    active_biome_source: Option<Res<ActiveBiomeSource>>,
    biome_registry: Option<Res<RegistrySnapshot<Biome>>>,
    saved: Option<Res<SavedColumns>>,
    mut cached_biome_registry: Local<Option<Arc<RegistrySnapshot<Biome>>>>,
) {
    let task_pool = CHUNK_TASK_POOL.get().unwrap();

    // Calculate how many columns we can dispatch this tick
    let current_in_flight = scheduler.in_flight.len();
    let max_in_flight = scheduler.config.max_in_flight;
    let max_dispatch = scheduler.config.max_dispatch_per_tick;

    // Early exit if at capacity
    if current_in_flight >= max_in_flight {
        return;
    }

    let available_capacity = max_in_flight - current_in_flight;
    let pending_count = scheduler.pending.len();

    // Dispatch up to min(available_capacity, max_dispatch, pending_count) columns
    let dispatch_count = available_capacity.min(max_dispatch).min(pending_count);

    if dispatch_count == 0 {
        return;
    }

    // Snapshot the biome context once per dispatch batch to avoid repeated deref.
    // Both resources must be present for Beta biome fill to activate.
    //
    // The registry snapshot is expensive to clone (it carries pre-serialized NBT
    // for every entry), so the deep clone happens only when the underlying
    // resource changes; subsequent ticks reuse the cached `Arc` handle.
    let biome_registry_arc = biome_registry.as_ref().map(|reg| {
        if reg.is_changed() || cached_biome_registry.is_none() {
            *cached_biome_registry = Some(Arc::new(RegistrySnapshot::clone(reg)));
        }
        cached_biome_registry.as_ref().unwrap().clone()
    });
    let biome_snapshot = biome_registry_arc.clone();
    let biome_context: Option<(Arc<BiomeSource>, Arc<RegistrySnapshot<Biome>>)> =
        match (active_biome_source.as_deref(), biome_registry_arc) {
            (Some(src), Some(reg)) => Some((src.0.clone(), reg)),
            _ => None,
        };

    let mut dispatched = 0usize;

    // Pop columns from priority queue in order (lowest distance first)
    while dispatched < dispatch_count {
        // Pop the first (lowest priority key) entry from the BTreeMap
        let Some((key, mut pending_column)) = scheduler.pending.pop_first() else {
            break;
        };

        let col = key.chunk_column_pos;

        // Remove from column index
        scheduler.priority_index.remove(&col);

        // Sort sections by Y (bottom-to-top) for Y-boundary cache reuse
        pending_column.sections.sort_by_key(|(_, y)| *y);
        trace!(
            "Dispatching chunk column at ({:?}) for generation - {:?}",
            col, pending_column.sections
        );

        column_trace::mark(col, ColumnStage::Generating);

        // Prepare data for the async task
        let router = overworld_noise_router.0.clone();
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let biome_ctx = biome_context.clone();
        let block_definitions = blocks.0.clone();
        let saved = saved.as_deref().cloned().zip(biome_snapshot.clone());

        // Extract section data for the task
        let sections_data: Vec<(Entity, ChunkPos)> = pending_column
            .sections
            .iter()
            .map(|(entity, y)| (*entity, ChunkPos::new(col.x, *y, col.z)))
            .collect();

        let y_sections: Vec<i32> = pending_column.sections.iter().map(|(_, y)| *y).collect();

        // Spawn the generation task
        let task = task_pool.spawn(async move {
            let _column = info_span!("world::column_load").entered();
            let started = Instant::now();
            let router = router.as_ref();
            let biome_context = biome_ctx.as_ref().map(|(src, reg)| {
                (
                    src.as_ref() as &BiomeSource,
                    reg.as_ref() as &RegistrySnapshot<Biome>,
                )
            });

            let loaded = saved.and_then(|(saved, biomes)| {
                let chunk = {
                    let _read = info_span!("world::column_read_saved").entered();
                    saved.read(col.x, col.z)?
                };
                let _decode = info_span!("world::column_decode_saved").entered();
                match column_sections(&chunk, &y_sections, &block_definitions, &biomes) {
                    Ok(sections) => Some(sections),
                    Err(err) => {
                        error!(%err, x = col.x, z = col.z, "decoding a saved column");
                        None
                    }
                }
            });

            if let Some(sections) = loaded {
                return ColumnResult {
                    sections: sections_data
                        .into_iter()
                        .zip(sections)
                        .map(|((entity, pos), result)| (entity, pos, result))
                        .collect(),
                    source: ColumnSource::Saved,
                    work: started.elapsed(),
                };
            }

            let mut results = {
                let _gen = info_span!("world::column_gen").entered();
                generate_column(
                    col.x,
                    col.z,
                    &y_sections,
                    router,
                    biome_context,
                    &block_definitions,
                    &cancel_clone,
                )
            };

            // Beta surface pass: place surface/filler/bedrock blocks with a
            // single per-chunk RNG seeded from the chunk coords.
            if let Some((src, _)) = &biome_context
                && matches!(src, BiomeSource::Beta { .. })
            {
                let seed = (col.x as i64)
                    .wrapping_mul(341873128712)
                    .wrapping_add((col.z as i64).wrapping_mul(132897987541));
                let mut rng = LegacyRandom::new(seed as u64);
                apply_beta_surface(
                    &mut results,
                    &y_sections,
                    col.x * 16,
                    col.z * 16,
                    router,
                    src,
                    &block_definitions,
                    &mut rng,
                );

                let world_seed = router.world_seed() as i64;
                let cave_ids = BetaCaveBlockIds::resolve(&block_definitions);
                let cave_config = mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig {
                    air_state: cave_ids.air.into(),
                    lava_state: cave_ids.lava.into(),
                    stone_state: cave_ids.stone.into(),
                    dirt_state: cave_ids.dirt.into(),
                    grass_state: cave_ids.grass.into(),
                    water_state: cave_ids.water.into(),
                    stationary_water_state: cave_ids.stationary_water.into(),
                    lava_level: 10,
                    range: 8,
                    horizontal_radius_multiplier: 1.0,
                    vertical_radius_multiplier: 1.0,
                };
                apply_beta_caves(
                    &mut results,
                    &y_sections,
                    col.x,
                    col.z,
                    world_seed,
                    &cave_config,
                    &cave_ids,
                );

                let ore_ids = BetaOreBlockIds::resolve(&block_definitions);
                apply_beta_ores(
                    &mut results,
                    &y_sections,
                    col.x,
                    col.z,
                    world_seed,
                    &ore_ids,
                );
            }

            let column_sections = sections_data
                .into_iter()
                .zip(results)
                .map(|((entity, pos), result)| (entity, pos, result.map(generated_section)))
                .collect();

            ColumnResult {
                sections: column_sections,
                source: ColumnSource::Generated,
                work: started.elapsed(),
            }
        });

        // Create in-flight entry
        let in_flight_column = InFlightColumn::new(
            col,
            pending_column.sections,
            cancel,
            task,
            pending_column.queued,
        );

        // Add to in-flight tracking
        scheduler.in_flight_index.insert(col);
        scheduler.in_flight.push(in_flight_column);

        dispatched += 1;
    }

    if dispatched > 0 {
        trace!("Dispatched generation tasks for {} columns", dispatched);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use mcrs_minecraft_world::block::definition::schema::PropertyValue;
    use mcrs_voxel_world::entity::player::chunk_view::ChunkTrackingView;

    fn corpus() -> &'static Blocks {
        static CORPUS: OnceLock<Blocks> = OnceLock::new();
        CORPUS.get_or_init(|| {
            let mut app = App::new();
            app.add_plugins(bevy_app::TaskPoolPlugin::default());
            app.add_plugins(bevy_asset::AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            });
            let asset_server = app.world().resource::<bevy_asset::AssetServer>().clone();
            let (definitions, _) =
                mcrs_minecraft_world::block::definition::load_block_definitions(&asset_server)
                    .expect("the block definition corpus loads");
            Blocks(Arc::new(definitions))
        })
    }

    fn noise_settings_state(field: &str) -> ProtoBlockState {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/overworld.json"
        );
        let settings: serde_json::Value = serde_json::from_slice(
            &std::fs::read(path).expect("the overworld noise settings ship"),
        )
        .expect("the noise settings parse");
        serde_json::from_value(settings[field].clone()).expect("the state parses")
    }

    #[test]
    fn the_terrain_block_and_the_sea_come_from_the_noise_settings() {
        let blocks = corpus();

        let stone = resolve_state(blocks, &noise_settings_state("default_block"));
        assert_eq!(
            stone,
            blocks.block("minecraft:stone").unwrap().default_state_id
        );

        let water = resolve_state(blocks, &noise_settings_state("default_fluid"));
        assert_eq!(
            water,
            blocks.block("minecraft:water").unwrap().default_state_id
        );
    }

    #[test]
    fn a_stated_property_moves_the_resolved_state() {
        let blocks = corpus();
        let water = blocks.block("minecraft:water").unwrap();
        let state = serde_json::from_str::<ProtoBlockState>(
            r#"{"id": "minecraft:water", "properties": {"level": "3"}}"#,
        )
        .expect("the state parses");
        assert_eq!(
            resolve_state(blocks, &state),
            water
                .with(water.default_state_id, "level", &PropertyValue::Int(3))
                .unwrap()
        );
    }

    #[test]
    fn worldgen_ingest_set_variants_compile() {
        // Sanity check that the SystemSet variant exists with the expected
        // shape; matched by the lighting plugin's `.after(...)` ordering.
        let _ = WorldgenIngestSet::ProcessCompletedColumns;
    }

    fn spawn_observer_with_view(app: &mut App, center: ChunkPos, distance: u8) -> Entity {
        let observer = PlayerChunkObserver {
            last_last_chunk_tracking_view: Some(ChunkTrackingView {
                center,
                distance,
                vert_distance: 8,
                min_section_y: i32::MIN,
                max_section_y: i32::MAX,
            }),
            ..PlayerChunkObserver::default()
        };
        app.world_mut().spawn(observer).id()
    }

    #[test]
    fn cancel_stale_columns_unloads_stale_sections() {
        let mut app = App::new();
        app.insert_resource(ColumnScheduler::default());
        app.add_systems(Update, cancel_stale_columns);

        spawn_observer_with_view(&mut app, ChunkPos::new(0, 0, 0), 2);

        let stale_pos = ChunkPos::new(100, 0, 100);
        let stale_section = app.world_mut().spawn((stale_pos, ChunkGenerating)).id();

        let stale_col = ColumnPos::new(stale_pos.x, stale_pos.z);
        let key = ColumnKey::new(0, stale_col);
        let pending = PendingColumn::new(vec![(stale_section, stale_pos.y)]);
        {
            let mut scheduler = app.world_mut().resource_mut::<ColumnScheduler>();
            scheduler.pending.insert(key, pending);
            scheduler.priority_index.insert(stale_col, key);
        }

        app.update();

        assert!(
            app.world().get::<ChunkUnloading>(stale_section).is_some(),
            "a stale section gets ChunkUnloading"
        );
        assert!(
            app.world().get::<ChunkGenerating>(stale_section).is_none(),
            "ChunkGenerating removed alongside the unload"
        );
    }
}
