use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::sync::Arc;
use std::time::Instant;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, section_states};
use mcrs_minecraft_engine_world::world::column_drain::{DrainWake, Finished, HandedOff, Handoff};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::LOADING_RINGS;
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_worldgen_generator::saved::SectionData;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::staging::{
    ColumnDelta, FilledSnapshot, Stage, StagingStore,
};
use mcrs_minecraft_worldgen_generator::task::{CancellationToken, ColumnSource};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;
use tracing::trace;

use crate::generation_link::{
    DeliveredColumn, DeliveredSection, GenerationReport, GenerationRequest, GeneratorInbox,
    RequestedSection, SectionBlocks, SectionContent, ViewUpdate, WantedColumn,
};

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

/// A column a view wants delivered, with the sections that asked for it.
pub(crate) struct PendingColumn {
    pub(crate) sections: Vec<RequestedSection>,
    /// When the column joined the queue, so a column that arrives late can say
    /// whether it waited for a worker or for the work itself.
    pub(crate) queued: Instant,
}

/// One stage of one column, in flight on the generation pool.
///
/// A halo column owns no entity, so nothing here names one: the store is keyed
/// by position alone, and the sections a delivery owes are read off the request
/// that wanted the column, at the moment it is delivered.
pub(crate) struct InFlightStage {
    pub(crate) col: ColumnPos,
    pub(crate) cancel: CancellationToken,
    pub(crate) task: HandedOff,
}

/// What a stage hands back. `None` from the task itself means cancelled.
pub(crate) enum StageResult {
    Filled(Box<FilledSnapshot>),
    Ran(usize, Vec<(ColumnPos, ColumnDelta)>),
    Merged(usize, Box<FilledSnapshot>),
}

/// Concurrency limits of a dimension's generation actor.
#[derive(Resource, Clone, Debug)]
pub(crate) struct SchedulerConfig {
    /// Maximum number of concurrent stage tasks. A column's fill and the run
    /// and merge of each of its rungs all share this budget, so it bounds the
    /// pool, not the column count.
    pub(crate) max_in_flight: usize,
    /// Maximum stages one dispatch pass hands to the pool. A pass runs after each batch of
    /// events the actor takes in.
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

/// Whether the dimension still listens to its actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Link {
    Open,
    Closed,
}

enum Event {
    Request(Result<GenerationRequest, flume::RecvError>),
    Finished(Result<Finished<Option<StageResult>>, flume::RecvError>),
}

/// The generation of one dimension's columns, on a thread of its own.
///
/// `pending` is the set of columns a view wants delivered, ordered by distance.
/// An entry stays there for the whole of its ladder, so sections ticketed after
/// its first stage was dispatched still reach the delivery. Everything the
/// ladder materialises — filled snapshots and the deltas one merge each
/// consumes — lives in `store`, keyed by position alone.
///
/// The actor blocks until the dimension asks for something or a stage finishes. Each
/// finished stage changes what its own 3×3 is owed, so the actor dispatches from there,
/// and a column that is newly wanted owes its whole shell; nothing walks every wanted
/// column again.
pub(crate) struct GenerationActor {
    inbox: GeneratorInbox,
    wake: Option<DrainWake>,
    pub(crate) traces: Option<ColumnTraceLog>,
    pub(crate) ctx: Option<FillContext>,
    pub(crate) pending: BTreeMap<ColumnKey, PendingColumn>,
    /// Reverse index: column position -> current priority key.
    pub(crate) priority_index: FxHashMap<ColumnPos, ColumnKey>,
    pub(crate) in_flight: Vec<InFlightStage>,
    pub(crate) handoff: Handoff<Option<StageResult>>,
    /// Columns whose stage changed since the last dispatch: they and their 3×3 may owe a
    /// stage now.
    pub(crate) touched: Vec<ColumnPos>,
    /// Wanted columns whose shells nobody has walked yet, nearest first.
    pub(crate) fresh: BTreeSet<ColumnKey>,
    /// Columns whose stage was cancelled since the last dispatch, and that may owe a fill again.
    pub(crate) forgotten: Vec<ColumnPos>,
    /// Columns whose stage moved, or that became wanted, since the last delivery: the ones
    /// that may have reached the top of the ladder.
    pub(crate) climbed: Vec<ColumnPos>,
    /// Columns wanted since the views were last checked against them.
    unscreened: Vec<ColumnPos>,
    views: ViewUpdate,
    /// The one owner of every materialised value of the ladder.
    pub(crate) store: StagingStore,
    pub(crate) config: SchedulerConfig,
}

impl GenerationActor {
    pub(crate) fn new(
        inbox: GeneratorInbox,
        wake: Option<DrainWake>,
        traces: Option<ColumnTraceLog>,
        config: SchedulerConfig,
    ) -> Self {
        Self {
            inbox,
            wake,
            traces,
            ctx: None,
            pending: BTreeMap::new(),
            priority_index: FxHashMap::default(),
            in_flight: Vec::new(),
            handoff: Handoff::default(),
            touched: Vec::new(),
            fresh: BTreeSet::new(),
            forgotten: Vec::new(),
            climbed: Vec::new(),
            unscreened: Vec::new(),
            views: ViewUpdate::default(),
            store: StagingStore::default(),
            config,
        }
    }

    /// Runs until the dimension drops its end of the link.
    pub(crate) fn run(mut self) {
        loop {
            let event = flume::Selector::new()
                .recv(&self.inbox.requests, Event::Request)
                .recv(self.handoff.receiver(), Event::Finished)
                .wait();
            match event {
                Event::Request(Ok(request)) => self.handle(request),
                Event::Finished(Ok(finished)) => self.finish(finished),
                Event::Request(Err(_)) | Event::Finished(Err(_)) => break,
            }
            if self.settle() == Link::Closed {
                break;
            }
        }
        for stage in &self.in_flight {
            stage.cancel.cancel();
        }
    }

    /// Takes in every event that is already waiting, dispatches what they made owed and
    /// delivers what reached the top of its ladder.
    pub(crate) fn settle(&mut self) -> Link {
        while let Ok(request) = self.inbox.requests.try_recv() {
            self.handle(request);
        }
        while let Ok(finished) = self.handoff.receiver().try_recv() {
            self.finish(finished);
        }
        self.dispatch();
        self.deliver()
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

    fn handle(&mut self, request: GenerationRequest) {
        match request {
            GenerationRequest::Context(ctx) => self.ctx = Some(*ctx),
            GenerationRequest::Want(wanted) => self.want(wanted),
            GenerationRequest::Views(update) => self.screen(update),
        }
    }

    /// A column stays in the queue for the whole of its ladder, so sections that
    /// arrive after its first stage was dispatched merge into the same entry and
    /// reach the same delivery.
    fn want(&mut self, wanted: WantedColumn) {
        let WantedColumn {
            col,
            distance_sq,
            sections,
        } = wanted;
        if let Some(key) = self.priority_index.get(&col) {
            if let Some(pending) = self.pending.get_mut(key) {
                pending.sections.extend(sections);
            }
            return;
        }
        column_trace::mark(self.traces.as_ref(), col, ColumnStage::Queued);
        let key = ColumnKey::new(distance_sq, col);
        self.pending.insert(
            key,
            PendingColumn {
                sections,
                queued: Instant::now(),
            },
        );
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
            let mut abandoned = Vec::new();
            for key in stale {
                if !self.within_views(key.chunk_column_pos, LOADING_RINGS) {
                    self.abandon(key, &mut abandoned);
                }
            }
            if views_moved || !abandoned.is_empty() {
                self.evict_outside_halo();
            }
            if !abandoned.is_empty() && self.report(GenerationReport::Abandoned(abandoned)) {
                self.wake();
            }
        }
        if previous.players != self.views.players && !self.views.players.is_empty() {
            self.reprioritize();
        }
    }

    fn within_views(&self, col: ColumnPos, slack: i32) -> bool {
        within(&self.views.views, col, slack)
    }

    fn abandon(&mut self, key: ColumnKey, abandoned: &mut Vec<Entity>) {
        trace!("Canceling stale column {:?}", key);
        column_trace::forget(self.traces.as_ref(), key.chunk_column_pos);
        self.priority_index.remove(&key.chunk_column_pos);
        self.fresh.remove(&key);
        if let Some(pending) = self.pending.remove(&key) {
            abandoned.extend(pending.sections.iter().map(|section| section.entity));
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
        for stage in &self.in_flight {
            if !within(views, stage.col, reach) {
                stage.cancel.cancel();
            }
        }
        // A stage still on the pool will write its answer into the store, so its
        // column must survive the eviction that runs beside its cancellation.
        self.store
            .retain(|col, stage| stage.in_flight() || within(views, col, reach));
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
            if let Some(pending) = self.pending.remove(&old) {
                self.pending.insert(new, pending);
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
    fn finish(&mut self, finished: Finished<Option<StageResult>>) {
        let Some(at) = self
            .in_flight
            .iter()
            .position(|stage| stage.task.id == finished.id)
        else {
            return;
        };
        let col = self.in_flight.swap_remove(at).col;
        let Some(result) = finished.output.flatten() else {
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
        let traces = self.traces.as_ref();
        match result {
            StageResult::Filled(snapshot) => {
                column_trace::mark(traces, col, ColumnStage::Filled);
                column_trace::set_source(traces, col, snapshot.source.label());
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
                column_trace::mark(traces, col, ColumnStage::Ran);
                for (target, delta) in deltas {
                    self.store.push_delta(col, rung, target, delta);
                }
                self.store.set_stage(col, Stage::Ran(rung as u8));
            }
            StageResult::Merged(rung, merged) => {
                column_trace::mark(traces, col, ColumnStage::Merged);
                self.store.insert_staged(col, rung, Arc::new(*merged));
                self.store.set_stage(col, Stage::Merged(rung as u8));
            }
        }
    }

    /// `Delivered`: the only point at which a column becomes visible.
    ///
    /// Every unit that could write into it has run and its delta has been merged,
    /// so nothing writes into the column after this.
    fn deliver(&mut self) -> Link {
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
        let mut delivered_any = false;
        for key in ready {
            let Some(column) = self.take_delivery(key) else {
                continue;
            };
            if !self.report(GenerationReport::Delivered(Box::new(column))) {
                return Link::Closed;
            }
            delivered_any = true;
        }
        if delivered_any {
            self.wake();
        }
        Link::Open
    }

    fn take_delivery(&mut self, key: ColumnKey) -> Option<DeliveredColumn> {
        let col = key.chunk_column_pos;
        let pending = self.pending.get(&key)?;
        let mut requested: Vec<(Entity, SectionPos)> = pending
            .sections
            .iter()
            .map(|section| (section.entity, SectionPos::new(col.x, section.y, col.z)))
            .collect();
        requested.sort_by_key(|(_, pos)| pos.y);
        let states = self.ctx.as_ref().map(|ctx| ctx.blocks.state_count());
        // Straight from the snapshot the store keeps, without a copy of the blocks between.
        let Some(merged) = self.store.merged(col) else {
            // The store was evicted under the request; the column keeps its place in the
            // queue and climbs the ladder again.
            self.store.forget(col);
            self.forgotten.push(col);
            return None;
        };
        let bottom = merged.y_sections.first().copied().unwrap_or(0);
        let sections: Vec<DeliveredSection> =
            carried_sections(&requested, &merged.sections, bottom)
                .map(|(entity, pos, section)| {
                    let content = match (section, states) {
                        (Some((blocks, biomes)), Some(states)) => {
                            SectionContent::Blocks(SectionBlocks {
                                blocks: ChunkBlocks::new(section_states(blocks, states)),
                                biomes: biomes.clone(),
                                ticks: merged
                                    .ticks
                                    .iter()
                                    .filter(|tick| SectionPos::from(tick.pos()) == pos)
                                    .copied()
                                    .collect(),
                            })
                        }
                        (Some(_), None) => SectionContent::Unchecked,
                        (None, _) => SectionContent::Missing,
                    };
                    DeliveredSection { entity, content }
                })
                .collect();
        let maps = merged.maps.clone();
        let source = merged.source;

        let pending = self.pending.remove(&key)?;
        self.priority_index.remove(&col);
        self.fresh.remove(&key);
        self.store.set_stage(col, Stage::Delivered);
        Some(DeliveredColumn {
            col,
            queued: pending.queued,
            source,
            maps,
            block_entities: self.store.take_block_entities(col),
            entities: self.store.take_entities(col),
            sections,
        })
    }

    /// Whether the dimension took the report.
    fn report(&self, report: GenerationReport) -> bool {
        self.inbox.reports.send(report).is_ok()
    }

    fn wake(&self) {
        if let Some(wake) = &self.wake {
            wake.wake();
        }
    }
}

struct KeyChange {
    old: ColumnKey,
    new: ColumnKey,
}

fn within(views: &[ChunkTrackingView], col: ColumnPos, slack: i32) -> bool {
    views.iter().any(|view| {
        let reach = i64::from(view.distance) + i64::from(slack);
        i64::from(col.x.abs_diff(view.center.x)) <= reach
            && i64::from(col.z.abs_diff(view.center.z)) <= reach
    })
}

/// The stage a column stands at once it owes nothing more: the merge of the
/// last rung, or `Filled` in a dimension that decorates nothing.
pub(crate) fn top_of_ladder(rungs: usize) -> Stage {
    match rungs.checked_sub(1) {
        Some(last) => Stage::Merged(last as u8),
        None => Stage::Filled,
    }
}

/// The sections a delivery owes, copied out of the whole column it generated.
/// `bottom` is the section the column buffer starts at.
pub(crate) fn carried_sections<'a>(
    sections_data: &'a [(Entity, SectionPos)],
    column: &'a [Option<SectionData>],
    bottom: i32,
) -> impl Iterator<Item = (Entity, SectionPos, Option<&'a SectionData>)> + 'a {
    sections_data.iter().map(move |&(entity, pos)| {
        let section = pos
            .y
            .checked_sub(bottom)
            .and_then(|slot| usize::try_from(slot).ok())
            .and_then(|slot| column.get(slot))
            .and_then(Option::as_ref);
        (entity, pos, section)
    })
}
