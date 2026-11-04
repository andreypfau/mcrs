use std::future::Future;

use bevy_tasks::TaskPool;
use mcrs_engine_link::Step;
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnStage;
use mcrs_minecraft_worldgen_generator::stages::{
    FillContext, fill_pooled, merge_column, run_region,
};
use mcrs_minecraft_worldgen_generator::staging::{Stage, StagingStore};
use mcrs_minecraft_worldgen_generator::task::CancellationToken;
use mcrs_voxel::ColumnPos;
use tracing::trace;

use crate::chunk_plugin::CHUNK_TASK_POOL;
use crate::generation_stage::{GenerationStage, InFlightStage, StageResult, top_of_ladder};

/// The next stage a column is owed, once its neighbourhood allows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owed {
    Fill,
    Run(usize),
    Merge(usize),
}

impl Owed {
    /// How near a wanted column has to be for this stage to be owed at all.
    fn reach(self, rungs: usize) -> i32 {
        let rings = |rungs: usize| i32::try_from(rungs).unwrap_or(i32::MAX).saturating_mul(2);
        let rung_reach = |rung: usize| rings(rungs.saturating_sub(rung.saturating_add(1)));
        match self {
            Owed::Fill => rings(rungs),
            Owed::Run(rung) => rung_reach(rung).saturating_add(1),
            Owed::Merge(rung) => rung_reach(rung),
        }
    }
}

fn owed(store: &StagingStore, v: ColumnPos, rungs: usize) -> Option<Owed> {
    let owed = match store.stage(v) {
        None => return Some(Owed::Fill),
        Some(Stage::Filled) if rungs > 0 => Owed::Run(0),
        Some(Stage::Merged(rung)) if usize::from(rung) + 1 < rungs => {
            Owed::Run(usize::from(rung) + 1)
        }
        Some(Stage::Ran(rung)) => Owed::Merge(usize::from(rung)),
        _ => return None,
    };
    let stage = store.stage(v)?;
    store.neighbourhood_reached(v, stage).then_some(owed)
}

/// Whether a dispatch pass may go on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    Going,
    Spent,
}

impl<T: Copy + Send + 'static> GenerationStage<T> {
    /// Dispatches what changed since the last pass made owed.
    ///
    /// The derived requests are computed here and nowhere stored: they are a
    /// function of what the views want and of what the store already holds. A
    /// wanted column at the top of the ladder needs its own 3×3 to have run the
    /// last rung, which needs the 5×5 to have merged the one below it, and so on
    /// down: every rung costs two rings, so the shell a wanted column derives is
    /// `2 × rungs` deep and each ring inside it is one phase further along.
    ///
    /// The stages that continue a ladder go first, so the columns already on their way
    /// are generated before the shells of further ones start. A column whose
    /// neighbourhood is not ready is not dispatched; the stage of a neighbour that
    /// finishes later touches it again.
    pub(crate) fn dispatch(&mut self, step: &mut Step<'_, Self>) {
        let Some(ctx) = self.ctx.clone() else {
            return;
        };
        let Some(pool) = CHUNK_TASK_POOL.get() else {
            tracing::error!("column generation runs before its task pool is built");
            return;
        };
        let budget = self
            .config
            .max_in_flight
            .saturating_sub(self.in_flight.len())
            .min(self.config.max_dispatch_per_pass);
        let mut remaining = budget;
        let mut dispatcher = Dispatcher {
            pool,
            ctx: &ctx,
            step,
        };
        if dispatcher.continue_ladders(self, &mut remaining) == Pass::Going {
            dispatcher.walk_fresh(self, &mut remaining);
        }
        if remaining < budget {
            trace!(
                dispatched = budget - remaining,
                staged = self.store.len(),
                in_flight = self.in_flight.len(),
                "dispatched column stages"
            );
        }
    }

    /// Whether a wanted column below `done` lies within `radius` of `col`.
    fn wanted_within(&self, col: ColumnPos, radius: i32, done: Stage) -> bool {
        col.square(radius).any(|u| {
            self.priority_index.contains_key(&u) && self.store.stage(u).is_none_or(|at| at < done)
        })
    }

    fn spawn<F>(
        &mut self,
        step: &mut Step<'_, Self>,
        pool: &TaskPool,
        col: ColumnPos,
        stage: Stage,
        work: impl FnOnce(CancellationToken) -> F,
    ) where
        F: Future<Output = Option<StageResult>> + Send + 'static,
    {
        let cancel = CancellationToken::new();
        let end = step.begin_task();
        let id = end.id();
        let work = work(cancel.clone());
        let task = pool.spawn(async move {
            // A cancelled fill drops its end unfinished, and the column drops off the ladder.
            if let Some(result) = work.await {
                end.finish(result);
            }
        });
        self.store.set_stage(col, stage);
        self.in_flight.push(id, InFlightStage { col, cancel }, task);
    }
}

struct Dispatcher<'a, 'step, T: Copy + Send + 'static> {
    pool: &'a TaskPool,
    ctx: &'a FillContext,
    step: &'a mut Step<'step, GenerationStage<T>>,
}

impl<T: Copy + Send + 'static> Dispatcher<'_, '_, T> {
    /// Dispatches what the columns whose stage changed, or was cancelled, made owed. What
    /// the budget does not cover waits for the next pass, which the next finished stage
    /// starts.
    fn continue_ladders(
        &mut self,
        generation: &mut GenerationStage<T>,
        remaining: &mut usize,
    ) -> Pass {
        let mut worklist = std::mem::take(&mut generation.forgotten);
        let fills = worklist.len();
        worklist.append(&mut generation.touched);
        let rungs = self.ctx.rungs();
        let done = top_of_ladder(rungs);
        let mut at = 0;
        while let Some(&v) = worklist.get(at) {
            at += 1;
            let owed = owed(&generation.store, v, rungs);
            // A fill is owed by being wanted, which a neighbour finishing a stage does not change.
            let owed = match owed {
                Some(Owed::Fill) if at > fills => continue,
                Some(owed) => owed,
                None => continue,
            };
            if !generation.wanted_within(v, owed.reach(rungs), done) {
                continue;
            }
            if *remaining == 0 {
                // `v` and everything after it waits for the next pass.
                let rest = at - 1;
                let split = fills.max(rest);
                generation
                    .forgotten
                    .extend_from_slice(worklist.get(rest..split).unwrap_or_default());
                generation
                    .touched
                    .extend_from_slice(worklist.get(split..).unwrap_or_default());
                return Pass::Spent;
            }
            self.dispatch(generation, remaining, v, owed);
            // A merge that advanced on the spot left its 3×3 owing something new already.
            worklist.append(&mut generation.touched);
        }
        Pass::Going
    }

    /// Walks the shells of the wanted columns nobody has walked yet, nearest first.
    fn walk_fresh(&mut self, generation: &mut GenerationStage<T>, remaining: &mut usize) {
        while let Some(&key) = generation.fresh.first() {
            if self.walk_shell(generation, remaining, key.chunk_column_pos) == Pass::Spent {
                return;
            }
            generation.fresh.remove(&key);
        }
    }

    /// Dispatches what the shell of the wanted column `u` owes, nearest rung first.
    fn walk_shell(
        &mut self,
        generation: &mut GenerationStage<T>,
        remaining: &mut usize,
        u: ColumnPos,
    ) -> Pass {
        let rungs = self.ctx.rungs();
        // A column at the top of its ladder derived everything it needed, or it
        // could not have got there.
        if generation
            .store
            .stage(u)
            .is_some_and(|at| at >= top_of_ladder(rungs))
        {
            return Pass::Going;
        }
        for v in u.square(Owed::Fill.reach(rungs)) {
            let Some(owed) = owed(&generation.store, v, rungs) else {
                continue;
            };
            if v.chebyshev_distance(u) > owed.reach(rungs) {
                continue;
            }
            if *remaining == 0 {
                return Pass::Spent;
            }
            self.dispatch(generation, remaining, v, owed);
        }
        Pass::Going
    }

    fn dispatch(
        &mut self,
        generation: &mut GenerationStage<T>,
        remaining: &mut usize,
        v: ColumnPos,
        owed: Owed,
    ) {
        let ctx = self.ctx;
        match owed {
            Owed::Fill => {
                column_trace::mark(generation.traces.as_ref(), v, ColumnStage::Generating);
                let ctx = ctx.clone();
                generation.spawn(
                    self.step,
                    self.pool,
                    v,
                    Stage::Filling,
                    |cancel| async move {
                        fill_pooled(&ctx, v, &cancel)
                            .map(|snapshot| StageResult::Filled(Box::new(snapshot)))
                    },
                );
            }
            Owed::Run(rung) => {
                let Some(region) = generation.store.region(v, rung) else {
                    return;
                };
                let ctx = ctx.clone();
                generation.spawn(self.step, self.pool, v, Stage::Running(rung as u8), |_| async move {
                    Some(StageResult::Ran(rung, run_region(&ctx, &region, rung)))
                });
            }
            Owed::Merge(rung) => {
                let Some(base) = generation.store.base(v, rung).cloned() else {
                    return;
                };
                let deltas = generation.store.deltas(v, rung);
                // A rung that grew nothing here leaves the column as it was, so
                // it advances on the spot and shares the snapshot it started
                // from rather than paying for a copy of the palettes.
                if deltas
                    .iter()
                    .all(|delta| delta.writes.is_empty() && delta.block_entities.is_empty())
                {
                    generation.store.insert_staged(v, rung, base);
                    generation.store.set_stage(v, Stage::Merged(rung as u8));
                    generation.touched.extend(v.square(1));
                    generation.climbed.push(v);
                    return;
                }
                let predicates = ctx.predicates.clone();
                let program = ctx.program.features.clone();
                generation.spawn(
                    self.step,
                    self.pool,
                    v,
                    Stage::Merging(rung as u8),
                    |_| async move {
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
                    },
                );
            }
        }
        *remaining = remaining.saturating_sub(1);
    }
}
