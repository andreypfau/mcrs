use bevy_tasks::{Task, TaskPool, TaskPoolBuilder};
use mcrs_engine_link::{InFlight, Step, TaskOutcome};
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine::stored_column::{StoredColumn, StoredSection};
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, section_states};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::LOADING_RINGS;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::staging::{
    ColumnDelta, FilledSnapshot, Stage, StagingStore,
};
use mcrs_minecraft_worldgen_generator::task::{CancellationToken, ColumnSource};
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;
use std::collections::BTreeSet;
use std::mem;
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use tracing::{trace, warn};

use crate::generated_ticks;
use crate::generation_input::{GenerationInput, ViewUpdate, WantedColumn};

/// The pool that every generation stage of the process runs its ladder stages on.
pub(crate) static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

/// Builds [`CHUNK_TASK_POOL`] unless it is built already.
pub(crate) fn init_chunk_task_pool(config: &SchedulerConfig) {
    CHUNK_TASK_POOL.get_or_init(|| {
        TaskPoolBuilder::new()
            .thread_name("ChunkGen".to_string())
            .num_threads(config.num_threads)
            .build()
    });
}

/// Sort key for the priority queue. Lower distance_sq values are dequeued first.
/// The column position (col_x, col_z) serves as a tiebreaker for determinism.
#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Debug)]
pub(crate) struct ColumnKey {
    /// Squared XZ distance to the nearest player. Primary sort key.
    pub(crate) distance_sq: i32,
    /// Column position. Tiebreaker for deterministic ordering.
    pub(crate) chunk_column_pos: ColumnPos,
}

impl ColumnKey {
    pub(crate) fn new(distance_sq: i32, chunk_column_pos: ColumnPos) -> Self {
        Self {
            distance_sq,
            chunk_column_pos,
        }
    }
}

/// One stage of one column, in flight on the generation pool.
///
/// A halo column owns no entity, so nothing here names one: the store is keyed
/// by position alone, and the sections a generated column owes are read off the
/// request that wanted it, at the moment it is emitted.
pub(crate) struct InFlightStage {
    pub(crate) col: ColumnPos,
    pub(crate) cancel: CancellationToken,
}

/// What a ladder stage hands back. A cancelled fill hands back nothing, and its task ends
/// dropped.
pub(crate) enum StageResult {
    Filled(Box<FilledSnapshot>),
    Ran(usize, Vec<(ColumnPos, ColumnDelta)>),
    Merged(usize, Box<FilledSnapshot>),
}

/// Concurrency limits of a dimension's generation stage.
#[derive(Clone, Debug)]
pub(crate) struct SchedulerConfig {
    /// Maximum number of concurrent stage tasks. A column's fill and the run
    /// and merge of each of its rungs all share this budget, so it bounds the
    /// pool, not the column count.
    pub(crate) max_in_flight: usize,
    /// Maximum stages one dispatch pass hands to the pool. A pass runs in each step of the
    /// generation stage.
    pub(crate) max_dispatch_per_pass: usize,
    /// Number of threads in the chunk generation thread pool.
    pub(crate) num_threads: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        Self {
            max_in_flight: parallelism.saturating_mul(8),
            max_dispatch_per_pass: 256,
            num_threads: (parallelism / 2).max(4),
        }
    }
}

/// The generation of one dimension's columns.
///
/// `pending` is the set of columns a view wants generated, ordered by distance.
/// An entry stays there for the whole of its ladder. Everything the
/// ladder materialises — filled snapshots and the deltas one merge each
/// consumes — lives in `store`, keyed by position alone.
///
/// Each finished ladder stage changes what its own 3×3 is owed, so the step dispatches from
/// there, and a column that is newly wanted owes its whole shell; nothing walks every wanted
/// column again.
pub(crate) struct GenerationStage {
    pub(crate) ctx: Option<FillContext>,
    /// The section rows of the dimension, which a generated column carries.
    rows: Range<i32>,
    pub(crate) pending: BTreeSet<ColumnKey>,
    /// Reverse index: column position -> current priority key.
    pub(crate) priority_index: FxHashMap<ColumnPos, ColumnKey>,
    pub(crate) in_flight: InFlight<InFlightStage, Task<()>>,
    /// Columns whose stage changed since the last dispatch: they and their 3×3 may owe a
    /// stage now.
    pub(crate) touched: Vec<ColumnPos>,
    /// Wanted columns whose shells nobody has walked yet, nearest first.
    pub(crate) fresh: BTreeSet<ColumnKey>,
    /// Columns whose stage was cancelled since the last dispatch, and that may owe a fill again.
    pub(crate) forgotten: Vec<ColumnPos>,
    /// Columns whose stage moved, or that became wanted, since the last emission: the ones
    /// that may have reached the top of the ladder.
    pub(crate) climbed: Vec<ColumnPos>,
    /// Columns wanted since the views were last checked against them.
    unscreened: Vec<ColumnPos>,
    views: ViewUpdate,
    /// Columns that left every view, emitted with the next step.
    abandoned: Vec<ColumnPos>,
    /// The one owner of every materialised value of the ladder.
    pub(crate) store: StagingStore,
    pub(crate) config: SchedulerConfig,
}

impl mcrs_engine_link::Stage for GenerationStage {
    const NAME: &'static str = "ColumnGen";

    type Input = GenerationInput;
    type Output = GenerationAnswer;
    type TaskOutput = StageResult;

    fn receive(&mut self, input: GenerationInput) {
        match input {
            GenerationInput::Context(ctx) => self.ctx = Some(*ctx),
            GenerationInput::Want(wanted) => self.want(wanted),
            GenerationInput::Views(update) => self.screen(update),
        }
    }

    fn finish(&mut self, outcome: TaskOutcome<StageResult>) {
        self.finish_stage(outcome);
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        for column in self.abandoned.drain(..) {
            step.emit(GenerationAnswer {
                column,
                stored: None,
            });
        }
        self.dispatch(step);
        self.emit_generated(step);
    }

    /// A dispatch pass cut short by its own limit, with room left on the pool, is work in
    /// itself.
    fn has_work(&self) -> bool {
        let dispatchable = self.ctx.is_some()
            && CHUNK_TASK_POOL.get().is_some()
            && self.in_flight.len() < self.config.max_in_flight
            && !(self.forgotten.is_empty() && self.touched.is_empty() && self.fresh.is_empty());
        dispatchable || !self.abandoned.is_empty() || !self.climbed.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.in_flight.is_empty() && self.abandoned.is_empty()
    }
}

impl Drop for GenerationStage {
    /// A fill on the pool stops at its next check of the token, rather than run to an end
    /// nobody takes.
    fn drop(&mut self) {
        for stage in self.in_flight.iter() {
            stage.cancel.cancel();
        }
    }
}

impl GenerationStage {
    pub(crate) fn new(config: SchedulerConfig, rows: Range<i32>) -> Self {
        Self {
            ctx: None,
            rows,
            pending: BTreeSet::new(),
            priority_index: FxHashMap::default(),
            in_flight: InFlight::default(),
            touched: Vec::new(),
            fresh: BTreeSet::new(),
            forgotten: Vec::new(),
            climbed: Vec::new(),
            unscreened: Vec::new(),
            views: ViewUpdate::default(),
            abandoned: Vec::new(),
            store: StagingStore::default(),
            config,
        }
    }

    pub(crate) fn rungs(&self) -> usize {
        self.ctx.as_ref().map_or(1, FillContext::rungs)
    }

    /// Check if a column has a stage on the pool.
    #[cfg(test)]
    pub(crate) fn is_in_flight(&self, col: ColumnPos) -> bool {
        self.store.stage(col).is_some_and(Stage::in_flight)
    }

    /// Drops a column from the store, as an eviction under the halo would, so it climbs
    /// the ladder again.
    #[cfg(test)]
    pub(crate) fn evict(&mut self, col: ColumnPos) {
        self.store.forget(col);
        self.forgotten.push(col);
    }

    /// A column stays in the queue for the whole of its ladder, so a second ask while it
    /// climbs gets the same generated column.
    fn want(&mut self, wanted: WantedColumn) {
        let WantedColumn { col, distance_sq } = wanted;
        if self.priority_index.contains_key(&col) {
            return;
        }
        let key = ColumnKey::new(distance_sq, col);
        self.pending.insert(key);
        self.priority_index.insert(col, key);
        self.fresh.insert(key);
        self.climbed.push(col);
        self.unscreened.push(col);
    }

    /// Cancels and evicts what has left every view, and reorders what is left.
    ///
    /// "Wanted" is widened by the halo: a column two away from one a view wants
    /// is still an input to its run, so it is neither cancelled nor evicted. The
    /// pending columns change only by a request and by the views, so with the same views
    /// only the columns wanted since the last update need a look.
    fn screen(&mut self, update: ViewUpdate) {
        let previous = mem::replace(&mut self.views, update);
        let unscreened = mem::take(&mut self.unscreened);
        let views_moved = previous.views != self.views.views;
        if !self.views.views.is_empty() {
            let stale: Vec<ColumnKey> = if views_moved {
                self.priority_index.values().copied().collect()
            } else {
                unscreened
                    .iter()
                    .filter_map(|col| self.priority_index.get(col).copied())
                    .collect()
            };
            let mut abandoned = false;
            for key in stale {
                if !self.within_views(key.chunk_column_pos, LOADING_RINGS) {
                    self.abandon(key);
                    abandoned = true;
                }
            }
            if views_moved || abandoned {
                self.evict_outside_halo();
            }
        }
        if previous.players != self.views.players && !self.views.players.is_empty() {
            self.reprioritize();
        }
    }

    fn within_views(&self, col: ColumnPos, slack: i32) -> bool {
        self.views.views.iter().any(|view| view.reaches(col, slack))
    }

    fn abandon(&mut self, key: ColumnKey) {
        trace!("Canceling stale column {:?}", key);
        let col = key.chunk_column_pos;
        self.priority_index.remove(&col);
        self.fresh.remove(&key);
        if self.pending.remove(&key) {
            self.abandoned.push(col);
        }
    }

    fn evict_outside_halo(&mut self) {
        // Every rung costs two rings: `Delivered(U)` needs the last rung run over
        // the 3×3 of `U`, which needs the one below it merged over the 5×5, and so
        // on down to the fill.
        let halo = i32::try_from(self.rungs())
            .unwrap_or(i32::MAX)
            .saturating_mul(2);
        let reach = LOADING_RINGS.saturating_add(halo);
        let views = &self.views.views;
        for stage in self.in_flight.iter() {
            if !views.iter().any(|view| view.reaches(stage.col, reach)) {
                stage.cancel.cancel();
            }
        }
        // A stage still on the pool will write its answer into the store, so its
        // column must survive the eviction that runs beside its cancellation.
        self.store.retain(|col, stage| {
            stage.in_flight() || views.iter().any(|view| view.reaches(col, reach))
        });
    }

    fn reprioritize(&mut self) {
        let players = &self.views.players;
        let updates: Vec<KeyChange> = self
            .priority_index
            .iter()
            .filter_map(|(&col, &old)| {
                let distance_sq = min_column_distance(&col, players);
                (distance_sq != old.distance_sq).then_some(KeyChange {
                    old,
                    new: ColumnKey::new(distance_sq, col),
                })
            })
            .collect();
        for KeyChange { old, new } in updates {
            if self.pending.remove(&old) {
                self.pending.insert(new);
                self.priority_index.insert(new.chunk_column_pos, new);
            }
            if self.fresh.remove(&old) {
                self.fresh.insert(new);
            }
        }
    }

    /// Puts what a stage produced into the store.
    ///
    /// A cancelled stage leaves nothing behind: the column drops off the ladder, and
    /// a later request refills it bit-identically, since a fill depends on nothing
    /// but the seed and the position.
    fn finish_stage(&mut self, outcome: TaskOutcome<StageResult>) {
        let Some(InFlightStage { col, .. }) = self.in_flight.retire(outcome.id()) else {
            return;
        };
        let TaskOutcome::Finished { output: result, .. } = outcome else {
            self.store.forget(col);
            self.forgotten.push(col);
            return;
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
        if self.store.stage(col) != Some(ticket) {
            return;
        }
        self.touched.extend(col.square(1));
        self.climbed.push(col);
        match result {
            StageResult::Filled(snapshot) => {
                let source = snapshot.source;
                self.store.insert_filled(*snapshot);
                // A column the save already holds was decorated before it was
                // written: its own ladder ends here, while its neighbours still
                // read it rung after rung.
                let stage = match source {
                    ColumnSource::Saved => {
                        let rungs = self.rungs();
                        self.store.insert_saved(col, rungs);
                        top_of_ladder(rungs)
                    }
                    ColumnSource::Generated => Stage::Filled,
                };
                self.store.set_stage(col, stage);
            }
            StageResult::Ran(rung, deltas) => {
                for (target, delta) in deltas {
                    self.store.push_delta(col, rung, target, delta);
                }
                self.store.set_stage(col, Stage::Ran(rung as u8));
            }
            StageResult::Merged(rung, merged) => {
                self.store.insert_staged(col, rung, Arc::new(*merged));
                self.store.set_stage(col, Stage::Merged(rung as u8));
            }
        }
    }

    /// `Delivered`: the only point at which a column becomes visible.
    ///
    /// Every unit that could write into it has run and its delta has been merged,
    /// so nothing writes into the column after this.
    fn emit_generated(&mut self, step: &mut Step<'_, Self>) {
        let done = top_of_ladder(self.rungs());
        let mut ready: Vec<ColumnKey> = self
            .climbed
            .drain(..)
            .filter_map(|col| {
                let key = self.priority_index.get(&col)?;
                self.store
                    .stage(col)
                    .is_some_and(|at| at >= done)
                    .then_some(*key)
            })
            .collect();
        ready.sort_unstable();
        ready.dedup();
        for key in ready {
            if let Some(answer) = self.take_generated(key) {
                step.emit(answer);
            }
        }
    }

    /// The column of `key` as the data layer keeps it, unlit, or no column when a section has
    /// no blocks.
    fn take_generated(&mut self, key: ColumnKey) -> Option<GenerationAnswer> {
        // A column reaches the top of its ladder only after the context arrived.
        let ctx = self.ctx.as_ref()?;
        let col = key.chunk_column_pos;
        if !self.pending.contains(&key) {
            return None;
        }
        let states = ctx.blocks.state_count();
        // Straight from the snapshot the store keeps, without a copy of the blocks between.
        let Some(merged) = self.store.merged(col) else {
            // The store was evicted under the request; the column keeps its place in the
            // queue and climbs the ladder again.
            self.store.forget(col);
            self.forgotten.push(col);
            return None;
        };
        let bottom = merged.y_sections.first().copied().unwrap_or(0);
        let sections: Option<Vec<StoredSection>> = self
            .rows
            .clone()
            .map(|y| {
                let (blocks, biomes) = y
                    .checked_sub(bottom)
                    .and_then(|slot| usize::try_from(slot).ok())
                    .and_then(|slot| merged.sections.get(slot))
                    .and_then(Option::as_ref)?;
                Some(StoredSection {
                    blocks: ChunkBlocks::new(section_states(blocks, states)),
                    biomes: biomes.clone(),
                    block_light: None,
                    sky_light: None,
                })
            })
            .collect();
        let heightmaps = merged.maps.clone();
        let ticks = generated_ticks::column_ticks(&merged.ticks, &ctx.blocks);

        self.pending.remove(&key);
        self.priority_index.remove(&col);
        self.fresh.remove(&key);
        self.store.set_stage(col, Stage::Delivered);
        // Nothing spawns generated entities yet, so they leave the store unspawned.
        self.store.take_entities(col);
        let block_entities = self.store.take_block_entities(col);
        let stored = match sections {
            Some(sections) => Some(StoredColumn {
                min_section_y: self.rows.start,
                sections,
                block_entities,
                heightmaps,
                ticks,
            }),
            None => {
                warn!(
                    x = col.x,
                    z = col.z,
                    "a generated column lacks the blocks of a section"
                );
                None
            }
        };
        Some(GenerationAnswer {
            column: col,
            stored,
        })
    }
}

struct KeyChange {
    old: ColumnKey,
    new: ColumnKey,
}

/// The stage a column stands at once it owes nothing more: the merge of the
/// last rung, or `Filled` in a dimension that decorates nothing.
pub(crate) fn top_of_ladder(rungs: usize) -> Stage {
    match rungs.checked_sub(1) {
        Some(last) => Stage::Merged(last as u8),
        None => Stage::Filled,
    }
}
