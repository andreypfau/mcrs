use bevy_tasks::{Task, TaskPool, TaskPoolBuilder};
use mcrs_engine_link::{InFlight, Step, TaskOutcome};
use mcrs_minecraft_anvil::{ChunkStructures, StructureStart};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::loading_rings::LOADING_RINGS;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, section_states};
use mcrs_minecraft_region_disk::stored_column::{StoredColumn, StoredSection};
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::staging::{
    ColumnDelta, FilledSnapshot, Stage, StagingStore,
};
use mcrs_minecraft_worldgen_generator::task::{CancellationToken, ColumnSource};
use mcrs_minecraft_worldgen_structure::piece::PieceContext;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;
use std::collections::BTreeSet;
use std::mem;
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use tracing::{error, trace, warn};

use crate::generated_ticks;
use crate::generation_input::GenerationInput;

pub(crate) static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

pub(crate) fn init_chunk_task_pool(config: &SchedulerConfig) {
    CHUNK_TASK_POOL.get_or_init(|| {
        TaskPoolBuilder::new()
            .thread_name("ChunkGen".to_string())
            .num_threads(config.num_threads)
            .build()
    });
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy, Debug)]
pub(crate) struct ColumnKey {
    pub(crate) distance_sq: i32,
    pub(crate) col: ColumnPos,
}

pub(crate) struct InFlightStage {
    pub(crate) col: ColumnPos,
    pub(crate) cancel: CancellationToken,
}

pub(crate) enum StageResult {
    Filled(Box<FilledSnapshot>),
    Ran(usize, Vec<(ColumnPos, ColumnDelta)>),
    Merged(usize, Box<FilledSnapshot>),
}

#[derive(Clone, Debug)]
pub(crate) struct SchedulerConfig {
    pub(crate) max_in_flight: usize,
    pub(crate) max_dispatch_per_pass: usize,
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

pub(crate) struct GenerationStage {
    pub(crate) ctx: Option<FillContext>,
    rows: Range<i32>,
    pub(crate) priority_index: FxHashMap<ColumnPos, ColumnKey>,
    pub(crate) in_flight: InFlight<InFlightStage, Option<Task<()>>>,
    pub(crate) touched: Vec<ColumnPos>,
    pub(crate) fresh: BTreeSet<ColumnKey>,
    pub(crate) forgotten: Vec<ColumnPos>,
    pub(crate) climbed: Vec<ColumnPos>,
    unscreened: Vec<ColumnPos>,
    views: Vec<ChunkTrackingView>,
    players: Vec<ColumnPos>,
    abandoned: Vec<ColumnPos>,
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
            GenerationInput::Want(col) => self.want(col),
            GenerationInput::Views(views) => self.screen(views),
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

    fn has_work(&self) -> bool {
        let dispatchable = self.ctx.is_some()
            && CHUNK_TASK_POOL.get().is_some()
            && self.in_flight.len() < self.config.max_in_flight
            && !(self.forgotten.is_empty() && self.touched.is_empty() && self.fresh.is_empty());
        dispatchable || !self.abandoned.is_empty() || !self.climbed.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.priority_index.is_empty() && self.in_flight.is_empty() && self.abandoned.is_empty()
    }
}

impl Drop for GenerationStage {
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
            priority_index: FxHashMap::default(),
            in_flight: InFlight::default(),
            touched: Vec::new(),
            fresh: BTreeSet::new(),
            forgotten: Vec::new(),
            climbed: Vec::new(),
            unscreened: Vec::new(),
            views: Vec::new(),
            players: Vec::new(),
            abandoned: Vec::new(),
            store: StagingStore::default(),
            config,
        }
    }

    pub(crate) fn rungs(&self) -> usize {
        self.ctx.as_ref().map_or(1, FillContext::rungs)
    }

    #[cfg(test)]
    pub(crate) fn is_in_flight(&self, col: ColumnPos) -> bool {
        self.store.stage(col).is_some_and(Stage::in_flight)
    }

    #[cfg(test)]
    pub(crate) fn evict(&mut self, col: ColumnPos) {
        self.store.forget(col);
        self.forgotten.push(col);
    }

    fn want(&mut self, col: ColumnPos) {
        if self.priority_index.contains_key(&col) {
            return;
        }
        let key = ColumnKey {
            distance_sq: min_column_distance(&col, &self.players),
            col,
        };
        self.priority_index.insert(col, key);
        self.fresh.insert(key);
        self.climbed.push(col);
        self.unscreened.push(col);
    }

    fn screen(&mut self, views: Vec<ChunkTrackingView>) {
        let views_moved = views != self.views;
        self.views = views;
        let unscreened = mem::take(&mut self.unscreened);
        if !self.views.is_empty() {
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
                if !self.within_views(key.col, LOADING_RINGS) {
                    self.abandon(key);
                    abandoned = true;
                }
            }
            if views_moved || abandoned {
                self.evict_outside_halo();
            }
        }
        let players = self.views.iter().map(|view| ColumnPos::from(view.center));
        if !players.clone().eq(self.players.iter().copied()) {
            self.players = players.collect();
            if !self.players.is_empty() {
                self.reprioritize();
            }
        }
    }

    fn within_views(&self, col: ColumnPos, slack: i32) -> bool {
        self.views.iter().any(|view| view.reaches(col, slack))
    }

    fn abandon(&mut self, key: ColumnKey) {
        trace!("Canceling stale column {:?}", key);
        let col = key.col;
        self.fresh.remove(&key);
        if self.priority_index.remove(&col).is_some() {
            self.abandoned.push(col);
        }
    }

    fn evict_outside_halo(&mut self) {
        let halo = i32::try_from(self.rungs())
            .unwrap_or(i32::MAX)
            .saturating_mul(2);
        let reach = LOADING_RINGS.saturating_add(halo);
        let views = &self.views;
        for stage in self.in_flight.iter() {
            if !views.iter().any(|view| view.reaches(stage.col, reach)) {
                stage.cancel.cancel();
            }
        }
        self.store.retain(|col, stage| {
            stage.in_flight() || views.iter().any(|view| view.reaches(col, reach))
        });
    }

    fn reprioritize(&mut self) {
        let players = &self.players;
        let updates: Vec<KeyChange> = self
            .priority_index
            .iter()
            .filter_map(|(&col, &old)| {
                let distance_sq = min_column_distance(&col, players);
                (distance_sq != old.distance_sq).then_some(KeyChange {
                    old,
                    new: ColumnKey { distance_sq, col },
                })
            })
            .collect();
        for KeyChange { old, new } in updates {
            self.priority_index.insert(new.col, new);
            if self.fresh.remove(&old) {
                self.fresh.insert(new);
            }
        }
    }

    fn finish_stage(&mut self, outcome: TaskOutcome<StageResult>) {
        let Some(InFlightStage { col, cancel }) = self.in_flight.retire(outcome.id()) else {
            return;
        };
        let TaskOutcome::Finished { output: result, .. } = outcome else {
            self.store.forget(col);
            if cancel.is_cancelled() {
                self.forgotten.push(col);
            } else {
                tracing::error!(?col, "the generation of a column panicked");
            }
            return;
        };
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

    fn take_generated(&mut self, key: ColumnKey) -> Option<GenerationAnswer> {
        let ctx = self.ctx.as_ref()?;
        let col = key.col;
        if self.priority_index.get(&col) != Some(&key) {
            return None;
        }
        let Some(merged) = self.store.merged(col).map(Arc::clone) else {
            self.store.forget(col);
            self.forgotten.push(col);
            return None;
        };
        self.priority_index.remove(&col);
        self.fresh.remove(&key);
        self.store.set_stage(col, Stage::Delivered);
        self.store.take_entities(col);
        let block_entities = self.store.take_block_entities(col);
        let stored = stored_column(ctx, col, &merged, self.rows.clone(), block_entities);
        if stored.is_none() {
            warn!(
                x = col.x,
                z = col.z,
                "a generated column lacks the blocks of a section or its heightmaps"
            );
        }
        Some(GenerationAnswer {
            column: col,
            stored,
        })
    }
}

pub fn stored_column(
    ctx: &FillContext,
    col: ColumnPos,
    merged: &FilledSnapshot,
    rows: Range<i32>,
    block_entities: Vec<GeneratedBlockEntity>,
) -> Option<StoredColumn> {
    let states = ctx.blocks.state_count();
    let bottom = merged.y_sections.first().copied().unwrap_or(0);
    let min_section_y = rows.start;
    let sections = rows
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
        .collect::<Option<Vec<_>>>()?;
    Some(StoredColumn {
        status: mcrs_minecraft_region_disk::stored_column::ChunkStatus::Full,
        is_light_on: false,
        inhabited_time: 0,
        last_update: 0,
        min_section_y,
        sections,
        block_entities,
        unknown_block_entities: Vec::new(),
        heightmaps: merged.maps.clone()?,
        retrogen: None,
        structures: structures_for(ctx, col),
        post_processing: Vec::new(),
        upgrade_data: Default::default(),
        blending_data: None,
        ticks: generated_ticks::column_ticks(&merged.ticks, &ctx.blocks),
    })
}

fn structures_for(ctx: &FillContext, column: ColumnPos) -> ChunkStructures {
    let Some(index) = ctx.structures.as_deref() else {
        return ChunkStructures::default();
    };
    let frozen = &index.tables().frozen;
    let mut structures = ChunkStructures::default();
    for (start_column, start) in index.starts_reaching(column) {
        let Some(structure) = frozen.structure(start.structure) else {
            continue;
        };
        let id = structure.id.clone();
        structures
            .references
            .entry(id.clone())
            .or_default()
            .insert(start_column);
        if start_column != column {
            continue;
        }
        let context = PieceContext {
            frozen,
            structure: start.structure,
        };
        let pieces = start
            .pieces
            .iter()
            .filter_map(|piece| match context.saved::<mcrs_minecraft_nbt::Error>(piece) {
                Ok(saved) => Some(saved),
                Err(error) => {
                    error!(structure = %id, %error, "a generated piece does not convert to NBT");
                    None
                }
            })
            .collect::<Vec<_>>();
        if !pieces.is_empty() {
            structures.starts.insert(
                id,
                StructureStart {
                    references: 0,
                    pieces,
                },
            );
        }
    }
    structures
}

struct KeyChange {
    old: ColumnKey,
    new: ColumnKey,
}

pub(crate) fn top_of_ladder(rungs: usize) -> Stage {
    match rungs.checked_sub(1) {
        Some(last) => Stage::Merged(last as u8),
        None => Stage::Filled,
    }
}
