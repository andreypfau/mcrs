use std::future::Future;

use bevy_tasks::TaskPool;
use mcrs_engine_link::Step;
use mcrs_engine_link::TaskEnd;
use mcrs_minecraft_worldgen_generator::stages::{
    FillContext, fill_pooled, merge_column, run_region,
};
use mcrs_minecraft_worldgen_generator::staging::{Stage, StagingStore};
use mcrs_minecraft_worldgen_generator::task::CancellationToken;
use mcrs_voxel::ColumnPos;
use tracing::trace;

use crate::generation_stage::{
    CHUNK_TASK_POOL, GenerationStage, InFlightStage, StageResult, top_of_ladder,
};

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

/// The side, as a power of two of chunks, of the squares whose owed fills share one pool task:
/// a worker reuses what it sampled for the columns beside one (`PERF.md`, "Fills in groups").
const FILL_GROUP_SHIFT: u32 = 2;

fn fill_group(col: ColumnPos) -> (i32, i32) {
    (col.x >> FILL_GROUP_SHIFT, col.z >> FILL_GROUP_SHIFT)
}

struct GroupFill {
    col: ColumnPos,
    cancel: CancellationToken,
    end: TaskEnd<StageResult>,
}

/// The fills of a group that have not started. Dropped by a panic of the fill before them,
/// they leave as cancelled fills, which the stage dispatches again.
struct Unstarted(std::vec::IntoIter<GroupFill>);

impl Drop for Unstarted {
    fn drop(&mut self) {
        for fill in self.0.by_ref() {
            fill.cancel.cancel();
        }
    }
}

/// Whether a dispatch pass may go on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    Going,
    Spent,
}

impl GenerationStage {
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
            fills: Vec::new(),
        };
        if dispatcher.continue_ladders(self, &mut remaining) == Pass::Going {
            dispatcher.walk_fresh(self, &mut remaining);
        }
        dispatcher.spawn_fills(self);
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
        self.in_flight
            .push(id, InFlightStage { col, cancel }, Some(task));
    }

    /// Fills `group` in one pool task, west to east and north to south, so each column finds
    /// the planes it borrows. A cancelled column drops out and the rest go on.
    fn spawn_fill_group(
        &mut self,
        step: &mut Step<'_, Self>,
        pool: &TaskPool,
        ctx: &FillContext,
        mut group: Vec<ColumnPos>,
    ) {
        group.sort_unstable_by_key(|col| (col.x, col.z));
        let mut fills = Vec::with_capacity(group.len());
        let mut stages = Vec::with_capacity(group.len());
        for col in group {
            let cancel = CancellationToken::new();
            let end = step.begin_task();
            let stage = InFlightStage {
                col,
                cancel: cancel.clone(),
            };
            stages.push((end.id(), stage));
            fills.push(GroupFill { col, cancel, end });
        }
        let ctx = ctx.clone();
        let mut task = Some(pool.spawn(async move {
            let mut unstarted = Unstarted(fills.into_iter());
            for fill in unstarted.0.by_ref() {
                if let Some(snapshot) = fill_pooled(&ctx, fill.col, &fill.cancel) {
                    fill.end.finish(StageResult::Filled(Box::new(snapshot)));
                }
            }
        }));
        let last = stages.len().saturating_sub(1);
        for (at, (id, stage)) in stages.into_iter().enumerate() {
            // The entry of the fill that ends last holds the task, so no entry that retires
            // while the group still fills drops it.
            let handle = if at == last { task.take() } else { None };
            self.in_flight.push(id, stage, handle);
        }
    }
}

struct Dispatcher<'a, 'step> {
    pool: &'a TaskPool,
    ctx: &'a FillContext,
    step: &'a mut Step<'step, GenerationStage>,
    /// The fills this pass found owed, in the order it found them: nearest first.
    fills: Vec<ColumnPos>,
}

impl Dispatcher<'_, '_> {
    /// Dispatches what the columns whose stage changed, or was cancelled, made owed. What
    /// the budget does not cover waits for the next pass, which the next finished stage
    /// starts.
    fn continue_ladders(
        &mut self,
        generation: &mut GenerationStage,
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
    fn walk_fresh(&mut self, generation: &mut GenerationStage, remaining: &mut usize) {
        while let Some(&key) = generation.fresh.first() {
            if self.walk_shell(generation, remaining, key.col) == Pass::Spent {
                return;
            }
            generation.fresh.remove(&key);
        }
    }

    /// Dispatches what the shell of the wanted column `u` owes, nearest rung first.
    fn walk_shell(
        &mut self,
        generation: &mut GenerationStage,
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

    /// Hands the fills of the pass to the pool, the group of the nearest column first. A group
    /// takes the other owed columns of its square with it, past the budget of the pass.
    fn spawn_fills(&mut self, generation: &mut GenerationStage) {
        let rungs = self.ctx.rungs();
        let done = top_of_ladder(rungs);
        let side = 1 << FILL_GROUP_SHIFT;
        let mut fills = std::mem::take(&mut self.fills);
        // A group fills one column after the other: with a thread for each fill, none waits.
        if generation.in_flight.len() + fills.len() <= generation.config.num_threads {
            for col in fills {
                generation.spawn_fill_group(self.step, self.pool, self.ctx, vec![col]);
            }
            return;
        }
        while let Some(&nearest) = fills.first() {
            let (group_x, group_z) = fill_group(nearest);
            let mut group: Vec<ColumnPos> = fills
                .extract_if(.., |col| fill_group(*col) == (group_x, group_z))
                .collect();
            for x in 0..side {
                for z in 0..side {
                    let col = ColumnPos::new(
                        (group_x << FILL_GROUP_SHIFT) + x,
                        (group_z << FILL_GROUP_SHIFT) + z,
                    );
                    if owed(&generation.store, col, rungs) == Some(Owed::Fill)
                        && generation.wanted_within(col, Owed::Fill.reach(rungs), done)
                    {
                        generation.store.set_stage(col, Stage::Filling);
                        group.push(col);
                    }
                }
            }
            generation.spawn_fill_group(self.step, self.pool, self.ctx, group);
        }
    }

    fn dispatch(
        &mut self,
        generation: &mut GenerationStage,
        remaining: &mut usize,
        v: ColumnPos,
        owed: Owed,
    ) {
        let ctx = self.ctx;
        match owed {
            Owed::Fill => {
                generation.store.set_stage(v, Stage::Filling);
                self.fills.push(v);
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
                                &predicates,
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
