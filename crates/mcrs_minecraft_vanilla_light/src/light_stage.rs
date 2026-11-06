use rustc_hash::FxHashMap;

use bevy_tasks::{AsyncComputeTaskPool, Task, TaskPool};
use mcrs_engine_link::{InFlight, Stage, Step, TaskOutcome};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_light_engine::epoch::{LightUpdate, PublishedLight, SectionLight};
use mcrs_minecraft_light_engine::level::SECTION_WIDTH;
use mcrs_minecraft_light_engine::queue::{DEFAULT_PRIORITY, LightQueue, Priority};
use mcrs_minecraft_light_engine::world::LightWorld;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};

use crate::light_budget::LightBudget;
use crate::light_input::LightInput;
use crate::light_output::{LightOutput, LitColumn};
use crate::pending_edits::PendingEdits;

/// How many section columns the light stage takes in before it hands out the next epochs.
///
/// Loading a column costs a 256 block column sky rescan. Only section loads and unloads are
/// held back: a block change takes effect the round it arrives, and there are never thousands
/// of those at once. The rescan is cheap next to the epochs it feeds, and holding it back only
/// starves them: a bulk load at render distance 96 admits its whole 38025 columns in under a
/// second of intake. The bound only keeps the first epochs from waiting behind the whole
/// intake.
pub const COLUMNS_PER_ROUND: usize = 1024;

/// The light of one dimension: the block and light data, the work on its way through the
/// world and the columns that wait for their light to settle.
///
/// Each step takes in the most urgent edits, hands the next epochs to the async compute pool,
/// and emits the light of the epochs that ended and the columns that settled.
pub(crate) struct LightStage {
    world: LightWorld,
    pending: PendingEdits,
    queue: LightQueue,
    /// The epochs in flight, each with the area it may still write into.
    epochs: InFlight<Epoch, Task<()>>,
    /// The epochs handed out so far, which numbers the next one.
    dispatched: u64,
    budget: LightBudget,
    awaiting: FxHashMap<ColumnPos, Settle>,
    /// Whether something happened since the last check that can settle an awaited column.
    settle_due: bool,
    /// Light published into the world and not emitted yet.
    relit: Vec<PublishedLight>,
    relit_output: RelitOutput,
    /// The columns the players stand in, as last told.
    players: Vec<ColumnPos>,
}

/// An epoch in flight: the area it may still write into, and its place in the order of
/// dispatch.
struct Epoch {
    area: BoundingBox,
    seq: u64,
}

/// How far an awaited column is from an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Settle {
    /// The column or a column around it is not loaded yet, or still has work queued.
    Draining,
    /// Every column around it is loaded and its queued work went out in the epochs before
    /// this one. Later epochs carry only work that arrived after that, so the answer does not
    /// wait for them: work that keeps arriving cannot hold a column back for ever.
    Retiring(u64),
}

/// What an awaited column gets in this step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Wait,
    Lit,
    /// The column left the stage, so its light never settles.
    Drop,
}

/// Whether a light stage hands back the light of each epoch and answers an awaited column by
/// its position, or hands back only the light of each awaited column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelitOutput {
    Emit,
    Drop,
}

impl Stage for LightStage {
    const NAME: &'static str = "light";

    type Input = LightInput;
    type Output = LightOutput;
    type TaskOutput = LightUpdate;

    fn receive(&mut self, input: LightInput) {
        match input {
            LightInput::Edits(edits) => {
                for edit in edits {
                    let priority = self.priority(edit.column());
                    self.pending.push_with_priority(edit, priority);
                }
            }
            LightInput::Players(players) => {
                self.players = players;
                self.reprioritize();
            }
            LightInput::AwaitColumns(columns) => {
                for column in columns {
                    self.awaiting.entry(column).or_insert(Settle::Draining);
                }
                self.settle_due = true;
            }
            LightInput::LightBudget(budget) => self.budget = budget,
        }
    }

    fn finish(&mut self, outcome: TaskOutcome<LightUpdate>) {
        let published = self.retire(outcome);
        if self.relit_output == RelitOutput::Emit {
            self.relit.extend(published);
        }
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        self.intake();
        // Without a task pool there is no worker to hand a batch to, so the queue keeps it
        // and the light lags until one exists.
        if let Some(pool) = AsyncComputeTaskPool::try_get() {
            self.dispatch(pool, step);
        }
        if !self.relit.is_empty() {
            step.emit(LightOutput::Relit(std::mem::take(&mut self.relit)));
        }
        // After the light it waited on, so the dimension has that light before it sends the
        // column.
        if !std::mem::take(&mut self.settle_due) {
            return;
        }
        match self.relit_output {
            RelitOutput::Emit => {
                let settled = self.take_settled();
                if !settled.is_empty() {
                    step.emit(LightOutput::Settled(settled));
                }
            }
            RelitOutput::Drop => {
                let lit = self.take_lit();
                if !lit.is_empty() {
                    step.emit(LightOutput::Lit(lit));
                }
            }
        }
    }

    /// Edits held back by the intake budget are work in themselves.
    fn has_work(&self) -> bool {
        !self.pending.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.queue.is_empty() && self.epochs.is_empty()
    }
}

impl LightStage {
    pub(crate) fn new(world: LightWorld) -> Self {
        Self {
            world,
            pending: PendingEdits::default(),
            queue: LightQueue::default(),
            epochs: InFlight::default(),
            dispatched: 0,
            budget: LightBudget::default(),
            awaiting: FxHashMap::default(),
            settle_due: false,
            relit: Vec::new(),
            relit_output: RelitOutput::Emit,
            players: Vec::new(),
        }
    }

    /// A stage that answers only the awaited columns, each with its light, for a world that
    /// takes nothing else from it.
    pub(crate) fn awaited_only(world: LightWorld) -> Self {
        Self {
            relit_output: RelitOutput::Drop,
            ..Self::new(world)
        }
    }

    /// Re-scores the waiting work against where the players stand now.
    ///
    /// A column is not sent until the light around it has settled, so work scored when it
    /// was raised carries the distance from wherever the player was then. One flight across
    /// the world and the queue is sorted by a position nobody occupies, which leaves the
    /// column under the player waiting behind thousands raised earlier and further away.
    fn reprioritize(&mut self) {
        if self.players.is_empty() || (self.pending.is_empty() && self.queue.is_empty()) {
            return;
        }
        let players = &self.players;
        let score = |column: ColumnPos| player_priority(column, players);
        self.pending.reprioritize(score);
        self.queue.reprioritize(score);
    }

    /// How urgent the light of `column` is: the same distance to the nearest player that the
    /// column scheduler treats as a ticket level, because a column is not sent until its light
    /// is published.
    fn priority(&self, column: ColumnPos) -> Priority {
        if self.players.is_empty() {
            return DEFAULT_PRIORITY;
        }
        player_priority(column, &self.players)
    }

    /// Publishes a finished epoch into the world and hands back what changed.
    fn retire(&mut self, outcome: TaskOutcome<LightUpdate>) -> Vec<PublishedLight> {
        if self.epochs.retire(outcome.id()).is_none() {
            return Vec::new();
        }
        self.settle_due = true;
        match outcome {
            TaskOutcome::Finished { output, .. } => self.world.apply(output),
            TaskOutcome::Dropped { .. } => {
                tracing::error!("a light epoch ended without its update");
                Vec::new()
            }
        }
    }

    /// Applies the most urgent edits to the world, up to the budget, and queues their
    /// lighting work.
    fn intake(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        // A column's edits go in one call so that a stack of section loads rescans
        // its 256 sky columns once rather than once per section — which is why the
        // budget counts columns and never splits one.
        for (priority, edits) in self.pending.take_admitted(COLUMNS_PER_ROUND) {
            for (column, influence) in self.world.apply_edits(edits) {
                self.queue.push_with_priority(column, influence, priority);
            }
        }
    }

    /// Hands the most urgent queued work, up to the budget, to `pool`.
    fn dispatch(&mut self, pool: &TaskPool, step: &mut Step<'_, Self>) {
        if self.queue.is_empty() {
            return;
        }
        let free = self
            .budget
            .epochs_in_flight
            .saturating_sub(self.epochs.len());
        if free == 0 {
            return;
        }
        let occupied: Vec<BoundingBox> = self.epochs.iter().map(|epoch| epoch.area).collect();
        for batch in self
            .queue
            .drain_batches(self.budget.cells_per_epoch, &occupied, free)
        {
            // A batch that yields no job leaves its columns owing nothing.
            self.settle_due = true;
            let Some(job) = self.world.prepare_batch(batch) else {
                continue;
            };
            let end = step.begin_task();
            let id = end.id();
            let epoch = Epoch {
                area: job.area(),
                seq: self.dispatched,
            };
            self.dispatched = self.dispatched.wrapping_add(1);
            let task = pool.spawn(async move {
                end.finish(job.run());
            });
            self.epochs.push(id, epoch, task);
        }
    }

    /// Takes the awaited columns whose light is lit, neighbours included, and lets go of the
    /// columns that left the stage.
    fn take_settled(&mut self) -> Vec<ColumnPos> {
        let mut awaiting = std::mem::take(&mut self.awaiting);
        let owed = self.owed();
        let mut settled = Vec::new();
        awaiting.retain(|&column, settle| match owed.answer(column, settle) {
            Answer::Wait => true,
            Answer::Lit => {
                settled.push(column);
                false
            }
            Answer::Drop => false,
        });
        self.awaiting = awaiting;
        settled
    }

    /// Takes the awaited columns whose light is lit, neighbours included, with that light.
    fn take_lit(&mut self) -> Vec<LitColumn> {
        let settled = self.take_settled();
        let bounds = self.world.bounds();
        settled
            .into_iter()
            .map(|column| LitColumn {
                column,
                sections: (bounds.min_section_y..=bounds.max_section_y)
                    .filter_map(|y| {
                        let pos = SectionPos::new(column.x, y, column.z);
                        self.world.section(pos).map(|section| SectionLight {
                            pos,
                            block_light: Some(section.block_light.clone()),
                            sky_light: Some(section.sky_light.clone()),
                        })
                    })
                    .collect(),
            })
            .collect()
    }

    fn owed(&self) -> Owed<'_> {
        Owed {
            world: &self.world,
            pending: &self.pending,
            queue: &self.queue,
            epochs: &self.epochs,
            dispatched: self.dispatched,
        }
    }
}

fn player_priority(column: ColumnPos, players: &[ColumnPos]) -> Priority {
    min_column_distance(&column, players).clamp(0, Priority::MAX as i32) as Priority
}

/// What the stage still owes, asked of the column a player waits on.
struct Owed<'a> {
    world: &'a LightWorld,
    pending: &'a PendingEdits,
    queue: &'a LightQueue,
    epochs: &'a InFlight<Epoch, Task<()>>,
    dispatched: u64,
}

impl Owed<'_> {
    /// Whether the light of one column is lit, neighbours included.
    ///
    /// A column is sent once and its light travels with it, so anything the engine still owes
    /// it would arrive too late. The neighbours count because a seam is lit against whatever
    /// stands beside it: a column whose neighbour is not loaded, or is still being lit, holds
    /// the darker edge that the neighbour's work is about to repair.
    fn answer(&self, column: ColumnPos, settle: &mut Settle) -> Answer {
        if !self.holds(column) {
            return Answer::Drop;
        }
        let cut = match *settle {
            Settle::Retiring(cut) => cut,
            Settle::Draining => {
                let drained = column.square(1).all(|near| {
                    !self.pending.holds(near) && self.queue.priority_of(near).is_none()
                }) && column.square(1).all(|near| self.loaded(near));
                if !drained {
                    return Answer::Wait;
                }
                *settle = Settle::Retiring(self.dispatched);
                self.dispatched
            }
        };
        let area = self.ring_area(column);
        if self
            .epochs
            .iter()
            .any(|epoch| epoch.seq < cut && epoch.area.intersects(area))
        {
            return Answer::Wait;
        }
        Answer::Lit
    }

    /// Whether the column is in the stage or on its way into it.
    fn holds(&self, column: ColumnPos) -> bool {
        self.pending.holds(column) || self.loaded(column)
    }

    /// A column loads and unloads whole, so its lowest section stands for it.
    fn loaded(&self, column: ColumnPos) -> bool {
        let bottom = self.world.bounds().min_section_y;
        self.world
            .section(SectionPos::new(column.x, bottom, column.z))
            .is_some()
    }

    fn ring_area(&self, column: ColumnPos) -> BoundingBox {
        let bounds = self.world.bounds();
        BoundingBox {
            min: BlockPos::new(
                column.x.saturating_sub(1).saturating_mul(SECTION_WIDTH),
                bounds.min_light_y(),
                column.z.saturating_sub(1).saturating_mul(SECTION_WIDTH),
            ),
            max: BlockPos::new(
                column
                    .x
                    .saturating_add(2)
                    .saturating_mul(SECTION_WIDTH)
                    .saturating_sub(1),
                bounds.max_light_y(),
                column
                    .z
                    .saturating_add(2)
                    .saturating_mul(SECTION_WIDTH)
                    .saturating_sub(1),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use mcrs_minecraft_chunk::{PalettedContainer, VoxelId, VoxelPalette};
    use mcrs_minecraft_light_engine::block::{LightProperties, LightRegistry, SpecialBlocks};
    use mcrs_minecraft_light_engine::level::LightBounds;
    use mcrs_minecraft_light_engine::world::Edit;
    use mcrs_voxel::{BlockStateId, SectionPos};

    const AIR: VoxelId = VoxelId(0);
    const TORCH: VoxelId = VoxelId(2);

    fn state_of(block: VoxelId) -> BlockStateId {
        BlockStateId::checked(block.0, 3).expect("a block of the test table")
    }

    fn light_stage() -> LightStage {
        let registry = Arc::new(LightRegistry::new(
            vec![
                LightProperties::AIR,
                LightProperties::SOLID,
                LightProperties::emitter(14),
            ],
            SpecialBlocks {
                unloaded: VoxelId(1),
                outside: AIR,
            },
        ));
        LightStage::new(LightWorld::new(registry, LightBounds::new(0, 1)))
    }

    fn load(column: ColumnPos) -> Vec<Edit> {
        (0..2)
            .map(|y| Edit::LoadSection {
                entity: 0,
                pos: SectionPos::new(column.x, y, column.z),
                blocks: Arc::new(VoxelPalette(PalettedContainer::Homogeneous(state_of(AIR)))),
            })
            .collect()
    }

    fn load_columns(stage: &mut LightStage, columns: i32) {
        let edits = (0..columns)
            .flat_map(|x| load(ColumnPos::new(x, 0)))
            .collect();
        stage.receive(LightInput::Edits(edits));
    }

    fn loaded(stage: &LightStage) -> usize {
        stage.world.loaded_sections().count()
    }

    #[test]
    fn intake_is_bounded_per_round_and_loses_nothing() {
        let mut stage = light_stage();
        let limit = COLUMNS_PER_ROUND;
        let columns = limit as i32 * 4 + 7;
        load_columns(&mut stage, columns);

        stage.intake();
        assert_eq!(loaded(&stage), limit * 2);
        assert!(stage.has_work());

        let mut rounds = 1;
        while stage.has_work() {
            stage.intake();
            rounds += 1;
            assert!(rounds < 100, "intake never drained");
        }
        assert_eq!(rounds, (columns as usize).div_ceil(limit));
        assert_eq!(loaded(&stage), columns as usize * 2);
    }

    #[test]
    fn a_block_change_is_not_deferred_behind_a_bulk_load() {
        let mut stage = light_stage();
        let limit = COLUMNS_PER_ROUND;
        let pos = BlockPos::new(8, 8, 8);
        load_columns(&mut stage, 1);
        stage.intake();

        load_columns(&mut stage, limit as i32 * 3);
        stage.receive(LightInput::Edits(vec![Edit::SetBlock {
            pos,
            block: state_of(TORCH),
        }]));
        stage.intake();

        assert!(stage.has_work());
        assert_eq!(stage.world.block_at(pos), TORCH);
    }

    #[test]
    fn the_most_urgent_column_is_admitted_before_the_backlog() {
        let mut stage = light_stage();
        let limit = COLUMNS_PER_ROUND;
        load_columns(&mut stage, limit as i32 * 2);

        let urgent = ColumnPos::new(-1, 0);
        stage.receive(LightInput::Players(vec![urgent]));
        stage.receive(LightInput::Edits(load(urgent)));
        stage.intake();

        assert!(
            stage
                .world
                .section(SectionPos::new(urgent.x, 0, urgent.z))
                .is_some()
        );
        assert!(stage.has_work());
    }

    /// A seam is lit against whatever stands beside it, so a column is only
    /// finished once its neighbours are loaded and lit.
    #[test]
    fn a_column_is_lit_only_once_the_eight_around_it_are_loaded_and_lit() {
        let column = ColumnPos::new(0, 0);
        let last = ColumnPos::new(1, 1);
        let mut stage = light_stage();
        let ring = column.square(1).filter(|near| *near != last);
        stage.receive(LightInput::Edits(ring.flat_map(load).collect()));
        stage.intake();
        stage.queue = LightQueue::default();
        stage.receive(LightInput::AwaitColumns(vec![column]));
        assert!(
            stage.take_settled().is_empty(),
            "a neighbour that is not loaded yet changes this column's seam"
        );

        stage.receive(LightInput::Edits(load(last)));
        stage.intake();
        assert!(
            stage.take_settled().is_empty(),
            "the neighbour's work will repair this column's seam"
        );

        stage.queue = LightQueue::default();
        assert_eq!(stage.take_settled(), [column]);
    }

    #[test]
    fn a_column_that_leaves_the_stage_is_never_answered() {
        let column = ColumnPos::new(0, 0);
        let mut stage = light_stage();
        stage.receive(LightInput::AwaitColumns(vec![column]));
        assert!(stage.take_settled().is_empty());
        assert!(
            stage.awaiting.is_empty(),
            "a column that is not in the stage would wait for ever"
        );
    }
}
