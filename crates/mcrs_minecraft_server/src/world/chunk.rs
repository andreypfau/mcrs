use crate::world::block_entity::spawn_block_entities;
use crate::world::entity::player::column_view::ColumnView;
use crate::world::heightmap::PendingColumnHeightmaps;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Local, MessageReader, ParamSet, Query, Resource, With, resource_exists};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Res, ResMut};
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::lifecycle::level::FULL_LEVEL;
use mcrs_minecraft_level::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_level::world::lifecycle::ticket::Ticket;
use mcrs_minecraft_level::world::lifecycle::trace as column_trace;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnStage;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_world::worldgen::beta_biome::BetaBiomeSourcePlugin;
use mcrs_minecraft_worldgen_generator::saved::SectionData;
use mcrs_minecraft_worldgen_generator::stages::{
    FillContext, fill_pooled, merge_column, run_region,
};
use mcrs_minecraft_worldgen_generator::staging::{
    ColumnDelta, FilledSnapshot, Stage, StagingStore,
};
use mcrs_minecraft_worldgen_generator::task::{CancellationToken, ColumnSource};
use rustc_hash::FxHashMap;
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tracing::{info, trace};

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BetaBiomeSourcePlugin);
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
                dispatch_column_generation.run_if(resource_exists::<FillContext>),
            )
                .chain()
                .after(mcrs_minecraft_level::world::lifecycle::ticket::ChunkSpawnSet),
        );
    }
}

pub(crate) static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

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

/// One stage of one column, in flight on the generation pool.
///
/// A halo column owns no entity, so nothing here names one: the store is keyed
/// by position alone, and the sections a delivery owes are read off the request
/// that wanted the column, at the moment it is delivered.
pub struct InFlightStage {
    pub col: ColumnPos,
    pub cancel: CancellationToken,
    pub task: Task<Option<StageResult>>,
}

/// What a stage hands back. `None` from the task itself means cancelled.
pub enum StageResult {
    Filled(Box<FilledSnapshot>),
    Ran(usize, Vec<(ColumnPos, ColumnDelta)>),
    Merged(usize, Box<FilledSnapshot>),
}

/// Configuration for the chunk column scheduler.
///
/// Controls concurrency limits and dispatch rates for chunk generation tasks.
#[derive(Resource, Clone)]
pub struct SchedulerConfig {
    /// Maximum number of concurrent stage tasks. A column's fill and the run
    /// and merge of each of its rungs all share this budget, so it bounds the
    /// pool, not the column count.
    pub max_in_flight: usize,
    /// Maximum stages to dispatch per tick.
    pub max_dispatch_per_tick: usize,
    /// Number of threads in the chunk generation thread pool.
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
/// `pending` is the set of columns a view wants delivered, ordered by distance.
/// An entry stays there for the whole of its ladder, so sections ticketed after
/// its first stage was dispatched still reach the delivery. Everything the
/// ladder materialises — filled snapshots and the deltas one merge each
/// consumes — lives in `store`, outside the ECS, keyed by position alone.
#[derive(Resource, Default)]
pub struct ColumnScheduler {
    /// Columns a view asked for. Lower `ColumnKey` values are worked first.
    pub pending: BTreeMap<ColumnKey, PendingColumn>,
    /// Reverse index: column position -> current priority key.
    pub priority_index: FxHashMap<ColumnPos, ColumnKey>,
    /// Active stage tasks with their cancellation tokens.
    pub in_flight: Vec<InFlightStage>,
    /// The one owner of every materialised value of the ladder.
    pub store: StagingStore,
    /// Configuration for concurrency limits and dispatch rates.
    pub config: SchedulerConfig,
}

impl ColumnScheduler {
    /// Check if a column is wanted and not yet delivered.
    pub fn is_pending(&self, col: ColumnPos) -> bool {
        self.priority_index.contains_key(&col)
    }

    /// Check if a column has a stage on the pool.
    pub fn is_in_flight(&self, col: ColumnPos) -> bool {
        self.store.stage(col).is_some_and(Stage::in_flight)
    }

    fn spawn<F>(
        &mut self,
        pool: &TaskPool,
        col: ColumnPos,
        stage: Stage,
        work: impl FnOnce(CancellationToken) -> F,
    ) where
        F: Future<Output = Option<StageResult>> + Send + 'static,
    {
        let cancel = CancellationToken::new();
        let task = pool.spawn(work(cancel.clone()));
        self.store.set_stage(col, stage);
        self.in_flight.push(InFlightStage { col, cancel, task });
    }
}

/// A column slower than `threshold` from queue to hand-off gets a line naming the
/// stage that cost the time. `MCRS_SLOW_CHUNK_MS` moves the bar.
pub(crate) struct SlowColumns {
    threshold: Duration,
    reported: Option<Instant>,
    unreported: u64,
}

impl Default for SlowColumns {
    fn default() -> Self {
        let ms = std::env::var("MCRS_SLOW_CHUNK_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(250);
        Self {
            threshold: Duration::from_millis(ms),
            reported: None,
            unreported: 0,
        }
    }
}

/// The sections a delivery owes, copied out of the whole column it generated.
/// `bottom` is the section the column buffer starts at.
pub(crate) fn carried_sections(
    sections_data: &[(Entity, SectionPos)],
    column: &[Option<SectionData>],
    bottom: i32,
) -> Vec<(Entity, SectionPos, Option<SectionData>)> {
    sections_data
        .iter()
        .map(|&(entity, pos)| {
            let section = usize::try_from(pos.y - bottom)
                .ok()
                .and_then(|slot| column.get(slot))
                .cloned()
                .flatten();
            (entity, pos, section)
        })
        .collect()
}

/// Squared XZ (column) distance from a chunk to the nearest player.
pub(crate) fn min_column_distance(pos: &ColumnPos, players: &[ColumnPos]) -> i32 {
    if players.is_empty() {
        return 0;
    }
    players
        .iter()
        .map(|p| pos.distance_squared(*p))
        .min()
        .unwrap_or(0)
}

impl SlowColumns {
    /// The whole ladder's latency, reported once a second with the count of the
    /// columns the sample stands for.
    fn report(&mut self, col: ColumnPos, queued: Instant, sections: usize, source: ColumnSource) {
        let total = queued.elapsed();
        if total < self.threshold {
            return;
        }
        // A stall is a property of the whole load, not of one column, and a line per
        // column drowns out every other log on the server.
        self.unreported += 1;
        if self
            .reported
            .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        let others = std::mem::replace(&mut self.unreported, 0) - 1;
        self.reported = Some(Instant::now());
        info!(
            x = col.x,
            z = col.z,
            source = source.label(),
            total_ms = total.as_secs_f32() * 1000.0,
            sections = sections,
            others = others,
            "slow chunk column"
        );
    }
}

/// Poll the stages on the pool and put what they produced into the store.
///
/// A cancelled stage leaves nothing behind: the column drops off the ladder, and
/// a later request refills it bit-identically, since a fill depends on nothing
/// but the seed and the position.
pub(crate) fn process_completed_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    ctx: Option<Res<FillContext>>,
) {
    let rungs = ctx.map_or(1, |ctx| ctx.rungs());
    let mut done: Vec<(ColumnPos, Option<StageResult>)> = Vec::new();
    scheduler
        .in_flight
        .retain_mut(|stage| match block_on(future::poll_once(&mut stage.task)) {
            Some(result) => {
                done.push((stage.col, result));
                false
            }
            None => true,
        });

    for (col, result) in done {
        let Some(result) = result else {
            scheduler.store.forget(col);
            continue;
        };
        // The column may have been dropped, or sent back to `Filled` by the
        // drop of one it wrote into, while its stage was on the pool. What
        // comes back then answers a question the store has stopped asking, and
        // the column climbs again from where it now stands.
        let ticket = match &result {
            StageResult::Filled(..) => Stage::Filling,
            StageResult::Ran(rung, _) => Stage::Running(*rung as u8),
            StageResult::Merged(rung, _) => Stage::Merging(*rung as u8),
        };
        if scheduler.store.stage(col) != Some(ticket) {
            continue;
        }
        match result {
            StageResult::Filled(snapshot) => {
                column_trace::mark(col, ColumnStage::Filled);
                column_trace::set_source(col, snapshot.source.label());
                let source = snapshot.source;
                scheduler.store.insert_filled(*snapshot);
                // A column the save already holds was decorated before it was
                // written: its own ladder ends here, while its neighbours still
                // read it rung after rung.
                let stage = match source {
                    ColumnSource::Saved => {
                        scheduler.store.insert_saved(col, rungs);
                        top_of_ladder(rungs)
                    }
                    ColumnSource::Generated => Stage::Filled,
                };
                scheduler.store.set_stage(col, stage);
            }
            StageResult::Ran(rung, deltas) => {
                column_trace::mark(col, ColumnStage::Ran);
                for (target, delta) in deltas {
                    scheduler.store.push_delta(col, rung, target, delta);
                }
                scheduler.store.set_stage(col, Stage::Ran(rung as u8));
            }
            StageResult::Merged(rung, merged) => {
                column_trace::mark(col, ColumnStage::Merged);
                scheduler.store.insert_staged(col, rung, Arc::new(*merged));
                scheduler.store.set_stage(col, Stage::Merged(rung as u8));
            }
        }
    }
}

/// The stage a column stands at once it owes nothing more: the merge of the
/// last rung, or `Filled` in a dimension that decorates nothing.
pub(crate) fn top_of_ladder(rungs: usize) -> Stage {
    match rungs.checked_sub(1) {
        Some(last) => Stage::Merged(last as u8),
        None => Stage::Filled,
    }
}

/// `Delivered`: the only point at which a column becomes visible.
///
/// Every unit that could write into it has run and its delta has been merged,
/// so nothing writes into the column after this.
pub(crate) fn deliver_merged_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    section_dimensions: Query<&InDimension>,
    mut stages: SectionStages,
    ctx: Option<Res<FillContext>>,
    mut commands: Commands,
    mut slow: Local<SlowColumns>,
) {
    let done = top_of_ladder(ctx.map_or(1, |ctx| ctx.rungs()));
    let ready: Vec<ColumnKey> = scheduler
        .pending
        .keys()
        .copied()
        .filter(|key| {
            scheduler
                .store
                .stage(key.chunk_column_pos)
                .is_some_and(|at| at >= done)
        })
        .collect();

    for key in ready {
        let col = key.chunk_column_pos;
        // No merged column means the store was evicted under the request; the
        // column keeps its place in the queue and climbs the ladder again.
        let mut sections_data: Vec<(Entity, SectionPos)> = scheduler.pending[&key]
            .sections
            .iter()
            .map(|(entity, y)| (*entity, SectionPos::new(col.x, *y, col.z)))
            .collect();
        sections_data.sort_by_key(|(_, pos)| pos.y);
        let Some((carried, maps, source)) = scheduler.store.merged(col).map(|merged| {
            let bottom = merged.y_sections.first().copied().unwrap_or(0);
            (
                carried_sections(&sections_data, &merged.sections, bottom),
                merged.maps.clone(),
                merged.source,
            )
        }) else {
            scheduler.store.forget(col);
            continue;
        };

        let entry = scheduler
            .pending
            .remove(&key)
            .expect("the key was read off the queue this tick");
        scheduler.priority_index.remove(&col);
        scheduler.store.set_stage(col, Stage::Delivered);
        let block_entities = scheduler.store.take_block_entities(col);
        // Any live section of the column answers for the whole column. The
        // first-enqueued one may already have been despawned while the rest of
        // the column still stands, and taking it alone would drop the lot.
        match entry
            .sections
            .iter()
            .find_map(|(section, _)| section_dimensions.get(*section).ok())
        {
            Some(dim) => spawn_block_entities(&mut commands, *dim, block_entities),
            None => tracing::debug!(
                dropped = block_entities.len(),
                "a column with no live section delivered its block entities nowhere"
            ),
        }

        slow.report(col, entry.queued, entry.sections.len(), source);
        column_trace::mark(col, ColumnStage::Loaded);
        column_trace::set_source(col, source.label());
        if let Some(maps) = maps {
            pending_heightmaps.0.insert(col, maps);
        }

        for (entity, _pos, section) in carried {
            match section {
                Some((blocks, biomes)) => {
                    commands
                        .entity(entity)
                        .try_insert((ChunkBlocks::new(blocks), biomes));
                    // One cancelled while it generated keeps the blocks for a ticket that
                    // takes it back, but stays on its way out.
                    if stages.get(entity) == Some(SectionStage::Generating) {
                        stages.set(entity, SectionStage::Loaded);
                    }
                }
                None => stages.set(entity, SectionStage::Unloading),
            }
        }
    }
}

/// Enqueue the columns a view asked for.
///
/// A column stays in the queue for the whole of its ladder, so sections that
/// arrive after its first stage was dispatched merge into the same entry and
/// reach the same delivery.
pub(crate) fn enqueue_pending_columns(
    mut stages: ParamSet<(MessageReader<SectionStageChanged>, SectionStages)>,
    mut requested: Local<Vec<(Entity, SectionPos)>>,
    mut scheduler: ResMut<ColumnScheduler>,
    players: Query<&Transform, With<Player>>,
) {
    requested.extend(
        stages
            .p0()
            .read()
            .filter(|change| change.to == SectionStage::Loading)
            .map(|change| (change.section, change.pos)),
    );
    let mut section_stages = stages.p1();
    let mut columns: HashMap<ColumnPos, Vec<(Entity, i32)>> = HashMap::new();
    for (section, pos) in requested.drain(..) {
        if section_stages.get(section) != Some(SectionStage::Loading) {
            continue;
        }
        trace!(
            "Requested generation for chunk section at ({}, {}, {})",
            pos.x, pos.y, pos.z
        );
        section_stages.set(section, SectionStage::Generating);
        columns
            .entry(ColumnPos::new(pos.x, pos.z))
            .or_default()
            .push((section, pos.y));
    }
    if columns.is_empty() {
        return;
    }

    let player_positions: Vec<ColumnPos> = players
        .iter()
        .map(|t| ColumnPos::from(t.translation))
        .collect();

    for (col, sections) in columns {
        if scheduler.is_pending(col) {
            // Merge new sections into the existing entry so the column is
            // delivered with ALL its sections in a single batch.
            if let Some(&key) = scheduler.priority_index.get(&col)
                && let Some(pending) = scheduler.pending.get_mut(&key)
            {
                pending.sections.extend(sections);
            }
            continue;
        }

        column_trace::mark(col, ColumnStage::Queued);
        let distance_sq = min_column_distance(&col, &player_positions);
        let key = ColumnKey::new(distance_sq, col);
        scheduler.pending.insert(key, PendingColumn::new(sections));
        scheduler.priority_index.insert(col, key);
    }
}

/// Cancel and evict what has left every player's view.
///
/// "Wanted" is widened by the halo: a column two away from one a view wants
/// is still an input to its run, so it is neither cancelled nor evicted.
#[cfg(test)]
pub(crate) fn request_section(
    world: &mut bevy_ecs::world::World,
    dim: Entity,
    pos: SectionPos,
) -> Entity {
    let section = world
        .spawn((pos, InDimension(dim), SectionStage::Loading))
        .id();
    world.write_message(SectionStageChanged::spawned(
        section,
        pos,
        dim,
        SectionStage::Loading,
    ));
    section
}

fn cancel_stale_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    mut stages: SectionStages,
    ctx: Option<Res<FillContext>>,
    views: Query<&ColumnView>,
) {
    // Every rung costs two rings: `Delivered(U)` needs the last rung run over
    // the 3×3 of `U`, which needs the one below it merged over the 5×5, and so
    // on down to the fill.
    let halo = 2 * ctx.map_or(1, |ctx| ctx.rungs()) as i32;
    // A view's loading tickets load as many rings past it as their level takes to fall to
    // full, and a column that far out is loaded like any other.
    let rings = (FULL_LEVEL - Ticket::PLAYER_LOADING.level) as i32;

    let player_views: Vec<_> = views.iter().filter_map(ColumnView::view).collect();

    if player_views.is_empty() {
        return;
    }

    let wanted = |col: ColumnPos, slack: i32| {
        player_views.iter().any(|view| {
            let dx = (col.x - view.center.x).abs();
            let dz = (col.z - view.center.z).abs();
            dx <= view.distance as i32 + slack && dz <= view.distance as i32 + slack
        })
    };

    let stale: Vec<(ColumnKey, Vec<Entity>)> = scheduler
        .priority_index
        .iter()
        .filter_map(|(col, key)| {
            if wanted(*col, rings) {
                return None;
            }
            let pending = scheduler.pending.get(key)?;
            Some((*key, pending.sections.iter().map(|(e, _)| *e).collect()))
        })
        .collect();

    for (key, entities) in stale {
        trace!("Canceling stale column {:?}", key);
        column_trace::forget(key.chunk_column_pos);

        scheduler.pending.remove(&key);
        scheduler.priority_index.remove(&key.chunk_column_pos);

        for entity in entities {
            stages.set(entity, SectionStage::Unloading);
        }
    }

    for stage in &scheduler.in_flight {
        if !wanted(stage.col, rings + halo) {
            stage.cancel.cancel();
        }
    }

    // A stage still on the pool will write its answer into the store, so its
    // column must survive the eviction that runs beside its cancellation.
    scheduler
        .store
        .retain(|col, stage| stage.in_flight() || wanted(col, rings + halo));
}

/// Update column priorities when players move.
fn reprioritize_columns(
    mut scheduler: ResMut<ColumnScheduler>,
    players: Query<&Transform, With<Player>>,
) {
    if scheduler.pending.is_empty() {
        return;
    }

    let player_positions: Vec<ColumnPos> = players
        .iter()
        .map(|t| ColumnPos::from(t.translation))
        .collect();

    if player_positions.is_empty() {
        return;
    }

    let mut updates: Vec<(ColumnKey, ColumnKey)> = Vec::new();

    for (&col, &old_key) in &scheduler.priority_index {
        let new_distance_sq = min_column_distance(&col, &player_positions);
        if new_distance_sq != old_key.distance_sq {
            updates.push((old_key, ColumnKey::new(new_distance_sq, col)));
        }
    }

    for (old_key, new_key) in updates {
        if let Some(pending_column) = scheduler.pending.remove(&old_key) {
            scheduler.pending.insert(new_key, pending_column);
            scheduler
                .priority_index
                .insert(new_key.chunk_column_pos, new_key);
        }
    }
}

/// Every column within `radius` of `centre`, the centre itself included.
fn square(centre: ColumnPos, radius: i32) -> impl Iterator<Item = ColumnPos> {
    (-radius..=radius).flat_map(move |dx| {
        (-radius..=radius).map(move |dz| ColumnPos::new(centre.x + dx, centre.z + dz))
    })
}

/// Walk the wanted columns in priority order and dispatch whatever stage each
/// one is owed next.
///
/// The derived requests are computed here and nowhere stored: they are a
/// function of what the views want this tick and of what the store already
/// holds. A wanted column at the top of the ladder needs its own 3×3 to have
/// run the last rung, which needs the 5×5 to have merged the one below it, and
/// so on down: every rung costs two rings, so the shell a wanted column derives
/// is `2 × rungs` deep and each ring inside it is one phase further along.
///
/// Nothing here waits. A column whose neighbourhood is not ready yet is simply
/// not dispatched this pass, and the walk moves on to the next thing that is —
/// so a stalled neighbourhood costs a comparison, never a worker.
pub(crate) fn dispatch_column_generation(
    mut scheduler: ResMut<ColumnScheduler>,
    ctx: Res<FillContext>,
) {
    let task_pool = CHUNK_TASK_POOL.get().unwrap();

    let current_in_flight = scheduler.in_flight.len();
    let max_in_flight = scheduler.config.max_in_flight;
    if current_in_flight >= max_in_flight {
        return;
    }
    let budget = (max_in_flight - current_in_flight).min(scheduler.config.max_dispatch_per_tick);
    if budget == 0 || scheduler.pending.is_empty() {
        return;
    }
    let ctx = ctx.as_ref();

    let wanted: Vec<ColumnPos> = scheduler
        .pending
        .keys()
        .map(|key| key.chunk_column_pos)
        .collect();

    let rungs = ctx.rungs();
    let done = top_of_ladder(rungs);

    let mut dispatched = 0usize;
    'wanted: for u in wanted {
        // A column at the top of its ladder derived everything it needed, or it
        // could not have got there; it is leaving the queue at this tick's
        // delivery. On a settled view that is every pending column, and walking
        // their shells again is the whole cost of an idle tick.
        if scheduler.store.stage(u).is_some_and(|at| at >= done) {
            continue;
        }

        for v in square(u, (2 * rungs) as i32) {
            if scheduler.store.stage(v).is_some() {
                continue;
            }
            if dispatched == budget {
                break 'wanted;
            }
            column_trace::mark(v, ColumnStage::Generating);
            let ctx = ctx.clone();
            scheduler.spawn(task_pool, v, Stage::Filling, |cancel| async move {
                fill_pooled(&ctx, v, &cancel)
                    .map(|snapshot| StageResult::Filled(Box::new(snapshot)))
            });
            dispatched += 1;
        }

        for rung in 0..rungs {
            let below = match rung.checked_sub(1) {
                Some(under) => Stage::Merged(under as u8),
                None => Stage::Filled,
            };
            let reach = 2 * (rungs - 1 - rung) as i32;

            for v in square(u, reach + 1) {
                if scheduler.store.stage(v) != Some(below)
                    || !scheduler.store.neighbourhood_reached(v, below)
                {
                    continue;
                }
                if dispatched == budget {
                    break 'wanted;
                }
                let Some(region) = scheduler.store.region(v, rung) else {
                    continue;
                };
                let ctx = ctx.clone();
                scheduler.spawn(task_pool, v, Stage::Running(rung as u8), |_| async move {
                    Some(StageResult::Ran(rung, run_region(&ctx, &region, rung)))
                });
                dispatched += 1;
            }

            for v in square(u, reach) {
                if scheduler.store.stage(v) != Some(Stage::Ran(rung as u8))
                    || !scheduler
                        .store
                        .neighbourhood_reached(v, Stage::Ran(rung as u8))
                {
                    continue;
                }
                let Some(base) = scheduler.store.base(v, rung).cloned() else {
                    continue;
                };
                let deltas = scheduler.store.deltas(v, rung);
                // A rung that grew nothing here leaves the column as it was, so
                // it advances on the spot and shares the snapshot it started
                // from rather than paying for a copy of the palettes.
                if deltas
                    .iter()
                    .all(|delta| delta.writes.is_empty() && delta.block_entities.is_empty())
                {
                    scheduler.store.insert_staged(v, rung, base);
                    scheduler.store.set_stage(v, Stage::Merged(rung as u8));
                    continue;
                }
                if dispatched == budget {
                    break 'wanted;
                }
                let predicates = ctx.predicates.clone();
                let program = ctx.program.features.clone();
                scheduler.spawn(task_pool, v, Stage::Merging(rung as u8), |_| async move {
                    Some(StageResult::Merged(
                        rung,
                        Box::new(merge_column(
                            &base,
                            &deltas,
                            predicates.as_ref(),
                            program
                                .as_deref()
                                .map(|program| &program.world.has_block_entity),
                        )),
                    ))
                });
                dispatched += 1;
            }
        }
    }

    if dispatched > 0 {
        trace!(
            dispatched,
            staged = scheduler.store.len(),
            in_flight = scheduler.in_flight.len(),
            "dispatched column stages"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use mcrs_minecraft_block::definition::schema::PropertyValue;
    use mcrs_minecraft_level::entity::player::chunk_view::ChunkTrackingView;
    use mcrs_minecraft_worldgen_density::proto::BlockState as ProtoBlockState;
    use mcrs_minecraft_worldgen_generator::block_state::try_resolve_state;
    use mcrs_minecraft_worldgen_generator::tests::blocks as corpus;

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

        let stone = try_resolve_state(blocks, &noise_settings_state("default_block"));
        assert_eq!(
            stone,
            Some(blocks.block("minecraft:stone").unwrap().default_state_id)
        );

        let water = try_resolve_state(blocks, &noise_settings_state("default_fluid"));
        assert_eq!(
            water,
            Some(blocks.block("minecraft:water").unwrap().default_state_id)
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
            try_resolve_state(blocks, &state),
            water.with(water.default_state_id, "level", &PropertyValue::Int(3))
        );
    }

    fn spawn_observer_with_view(app: &mut App, center: SectionPos, distance: u8) -> Entity {
        let view = ColumnView::looking_at(ChunkTrackingView {
            center,
            distance,
            vert_distance: 8,
            min_section_y: i32::MIN,
            max_section_y: i32::MAX,
        });
        app.world_mut().spawn(view).id()
    }

    /// The ladder must produce the column the fill alone produces: with no
    /// features to run, every delta is empty and the merge is the identity.
    /// This drives the whole of it — the derived 5x5 of fills, the 3x3 of runs,
    /// the merge and the delivery — through the ECS, so a readiness or budget
    /// bug shows up as a column that never arrives.
    #[test]
    fn the_ladder_delivers_the_column_the_fill_produced() {
        use crate::world::heightmap::PendingColumnHeightmaps;
        use mcrs_minecraft_worldgen_generator::stages::fill_column;
        use mcrs_minecraft_worldgen_generator::tests::build_beta_router;

        CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());

        let router = Arc::new(build_beta_router());
        let col = ColumnPos::new(1, -2);
        let ctx = mcrs_minecraft_worldgen_generator::tests::bare_fill_context(router);
        let y_sections = ctx.y_sections.clone();

        let mut app = App::new();
        app.add_message::<SectionStageChanged>();
        app.insert_resource(ctx.clone());
        app.init_resource::<PendingColumnHeightmaps>();
        app.insert_resource(ColumnScheduler {
            config: SchedulerConfig {
                max_in_flight: 64,
                max_dispatch_per_tick: 64,
                num_threads: 2,
            },
            ..Default::default()
        });
        app.add_systems(
            Update,
            (
                enqueue_pending_columns,
                dispatch_column_generation,
                process_completed_columns,
                deliver_merged_columns,
            )
                .chain(),
        );

        let dim = app.world_mut().spawn_empty().id();
        let carried: Vec<i32> = vec![-1, 0, 4];
        let sections: Vec<Entity> = carried
            .iter()
            .map(|&y| request_section(app.world_mut(), dim, SectionPos::new(col.x, y, col.z)))
            .collect();
        let loaded = |app: &App, section: Entity| {
            app.world().get::<SectionStage>(section) == Some(&SectionStage::Loaded)
        };

        let mut delivered = false;
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            app.update();
            if sections.iter().all(|e| loaded(&app, *e)) {
                delivered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(delivered, "the ladder never delivered the column");

        let mut buffer = mcrs_minecraft_worldgen_generator::ColumnBlocks::new(&y_sections);
        let alone = fill_column(&ctx, col, &mut buffer, &CancellationToken::new())
            .expect("the fill was not cancelled");
        let bottom = y_sections[0];

        // A section ticketed after the column was delivered must be delivered
        // from the same merged blocks, not from the snapshot the neighbours read.
        let late = request_section(app.world_mut(), dim, SectionPos::new(col.x, 5, col.z));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline && !loaded(&app, late) {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            loaded(&app, late),
            "a section ticketed after the delivery was never delivered"
        );

        for (&y, entity) in carried
            .iter()
            .chain([&5].into_iter())
            .zip(sections.iter().chain([&late].into_iter()))
        {
            let want = alone.sections[(y - bottom) as usize]
                .as_ref()
                .expect("the fill produced this section");
            let got = app
                .world()
                .get::<ChunkBlocks>(*entity)
                .expect("a delivered section carries its blocks");
            for cell in 0..mcrs_minecraft_worldgen_generator::ColumnBlocks::SECTION_VOLUME {
                let local = mcrs_minecraft_core::LocalPos::from_index(cell);
                let (x, ly, z) = (local.x() as usize, local.y() as usize, local.z() as usize);
                assert_eq!(
                    got.get_cell(x, ly, z),
                    want.0.get_cell(x, ly, z),
                    "section {y} diverged at ({x}, {ly}, {z})"
                );
            }
        }
    }

    #[test]
    fn cancel_stale_columns_unloads_stale_sections() {
        let mut app = App::new();
        app.add_message::<SectionStageChanged>();
        app.insert_resource(ColumnScheduler::default());
        app.add_systems(Update, cancel_stale_columns);

        spawn_observer_with_view(&mut app, SectionPos::new(0, 0, 0), 2);

        let dim = app.world_mut().spawn_empty().id();
        let stale_pos = SectionPos::new(100, 0, 100);
        let stale_section = app
            .world_mut()
            .spawn((stale_pos, InDimension(dim), SectionStage::Generating))
            .id();

        let stale_col = ColumnPos::new(stale_pos.x, stale_pos.z);
        let key = ColumnKey::new(0, stale_col);
        let pending = PendingColumn::new(vec![(stale_section, stale_pos.y)]);
        {
            let mut scheduler = app.world_mut().resource_mut::<ColumnScheduler>();
            scheduler.pending.insert(key, pending);
            scheduler.priority_index.insert(stale_col, key);
        }

        app.update();

        assert_eq!(
            app.world().get::<SectionStage>(stale_section),
            Some(&SectionStage::Unloading),
            "a stale section is sent on its way out"
        );
    }
}
