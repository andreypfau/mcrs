use rustc_hash::FxHashSet;

use bevy_tasks::TaskPool;
use flume::Sender;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_light_engine::epoch::PublishedLight;
use mcrs_minecraft_light_engine::level::SECTION_WIDTH;
use mcrs_minecraft_light_engine::queue::{LightQueue, Priority};
use mcrs_minecraft_light_engine::world::LightWorld;
use mcrs_voxel::{BlockPos, ColumnPos};

use crate::intake_budget::IntakeBudget;
use crate::light_budget::LightBudget;
use crate::light_epoch::LightEpoch;
use crate::light_inbox::{FinishedEpoch, LightInbox};
use crate::pending_edits::PendingEdits;
use crate::to_light::ToLight;

/// Everything a light actor owns: the block and light data, the work on its way through the
/// world and the columns that wait for their light to settle.
pub(crate) struct LightState {
    world: LightWorld,
    pending: PendingEdits,
    queue: LightQueue,
    epoch: LightEpoch,
    budget: LightBudget,
    intake: IntakeBudget,
    awaiting: FxHashSet<ColumnPos>,
    /// Whether something happened since the last check that can settle an awaited column.
    settle_due: bool,
}

impl LightState {
    pub(crate) fn new(world: LightWorld) -> Self {
        Self {
            world,
            pending: PendingEdits::default(),
            queue: LightQueue::default(),
            epoch: LightEpoch::default(),
            budget: LightBudget::default(),
            intake: IntakeBudget::default(),
            awaiting: FxHashSet::default(),
            settle_due: false,
        }
    }

    pub(crate) fn receive(&mut self, message: ToLight) {
        match message {
            ToLight::Edits(edits) => {
                for queued in edits {
                    self.pending
                        .push_with_priority(queued.edit, queued.priority);
                }
            }
            ToLight::Players(players) => self.reprioritize(&players),
            ToLight::AwaitColumns(columns) => {
                self.awaiting.extend(columns);
                self.settle_due = true;
            }
            ToLight::LightBudget(budget) => self.budget = budget,
            ToLight::IntakeBudget(intake) => self.intake = intake,
        }
    }

    /// Re-scores the waiting work against where the players stand now.
    ///
    /// A column is not sent until the light around it has settled, so work scored when it
    /// was raised carries the distance from wherever the player was then. One flight across
    /// the world and the queue is sorted by a position nobody occupies, which leaves the
    /// column under the player waiting behind thousands raised earlier and further away.
    fn reprioritize(&mut self, players: &[ColumnPos]) {
        if players.is_empty() || (self.pending.is_empty() && self.queue.is_empty()) {
            return;
        }
        let score = |column: ColumnPos| player_priority(column, players);
        self.pending.reprioritize(score);
        self.queue.reprioritize(score);
    }

    /// Publishes a finished epoch into the world and hands back what changed.
    pub(crate) fn retire(&mut self, finished: FinishedEpoch) -> Vec<PublishedLight> {
        if !self.epoch.retire(finished.id) {
            return Vec::new();
        }
        self.settle_due = true;
        match finished.update {
            Some(update) => self.world.apply(update),
            None => {
                tracing::error!("a light epoch ended without its update");
                Vec::new()
            }
        }
    }

    /// Whether edits wait that the next [`Self::intake`] takes in.
    pub(crate) fn has_intake(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Applies the most urgent edits to the world, up to the budget, and queues their
    /// lighting work.
    pub(crate) fn intake(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        // A column's edits go in one call so that a stack of section loads rescans
        // its 256 sky columns once rather than once per section — which is why the
        // budget counts columns and never splits one.
        for (priority, edits) in self.pending.take_admitted(self.intake.columns_per_round) {
            for (column, influence) in self.world.apply_edits(edits) {
                self.queue.push_with_priority(column, influence, priority);
            }
        }
    }

    /// Hands the most urgent queued work, up to the budget, to `pool`.
    pub(crate) fn dispatch(&mut self, pool: &TaskPool, inbox: &Sender<LightInbox>) {
        if self.queue.is_empty() {
            return;
        }
        let free = self
            .budget
            .epochs_in_flight
            .saturating_sub(self.epoch.running_count());
        if free == 0 {
            return;
        }
        let occupied = self.epoch.areas();
        for batch in self
            .queue
            .drain_batches(self.budget.cells_per_epoch, &occupied, free)
        {
            // A batch that yields no job leaves its columns owing nothing.
            self.settle_due = true;
            let Some(job) = self.world.prepare_batch(batch) else {
                continue;
            };
            self.epoch.spawn(pool, inbox.clone(), job);
        }
    }

    /// Takes the awaited columns whose light has settled, neighbours included.
    pub(crate) fn take_settled(&mut self) -> Vec<ColumnPos> {
        if !std::mem::take(&mut self.settle_due) || self.awaiting.is_empty() {
            return Vec::new();
        }
        let owed = Owed {
            world: &self.world,
            pending: &self.pending,
            queue: &self.queue,
            epoch: &self.epoch,
        };
        let mut settled = Vec::new();
        self.awaiting.retain(|&column| {
            if owed.settled_around(column) {
                settled.push(column);
                false
            } else {
                true
            }
        });
        settled
    }

    /// Whether the actor owes no light for anything it was handed.
    pub(crate) fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.queue.is_empty() && !self.epoch.is_running()
    }
}

/// How urgent the light of `column` is: the same distance to the nearest player that the
/// column scheduler treats as a ticket level, because a column is not sent until its light
/// is published.
pub(crate) fn player_priority(column: ColumnPos, players: &[ColumnPos]) -> Priority {
    min_column_distance(&column, players).clamp(0, Priority::MAX as i32) as Priority
}

/// What the actor still owes, asked of the column a player waits on.
struct Owed<'a> {
    world: &'a LightWorld,
    pending: &'a PendingEdits,
    queue: &'a LightQueue,
    epoch: &'a LightEpoch,
}

impl Owed<'_> {
    /// Whether the light of one column is finished, neighbours included.
    ///
    /// A column is sent once and its light travels with it, so anything the
    /// engine still owes it would arrive too late. The neighbours count because
    /// a seam is lit against whatever stands beside it: a column whose
    /// neighbour is still being lit holds the darker edge that neighbour's work
    /// is about to repair.
    fn settled_around(&self, column: ColumnPos) -> bool {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let neighbour = ColumnPos::new(column.x + dx, column.z + dz);
                if self.pending.holds(neighbour) || self.queue.priority_of(neighbour).is_some() {
                    return false;
                }
            }
        }
        let bounds = self.world.bounds();
        let area = BoundingBox {
            min: BlockPos::new(
                (column.x - 1) * SECTION_WIDTH,
                bounds.min_light_y(),
                (column.z - 1) * SECTION_WIDTH,
            ),
            max: BlockPos::new(
                (column.x + 2) * SECTION_WIDTH - 1,
                bounds.max_light_y(),
                (column.z + 2) * SECTION_WIDTH - 1,
            ),
        };
        !self.epoch.touches(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use mcrs_minecraft_chunk::{PalettedContainer, VoxelId, VoxelPalette};
    use mcrs_minecraft_light_engine::block::{LightProperties, LightRegistry, SpecialBlocks};
    use mcrs_minecraft_light_engine::level::LightBounds;
    use mcrs_minecraft_light_engine::queue::DEFAULT_PRIORITY;
    use mcrs_minecraft_light_engine::world::Edit;
    use mcrs_voxel::{BlockStateId, SectionPos};

    use crate::to_light::QueuedEdit;

    const AIR: VoxelId = VoxelId(0);
    const TORCH: VoxelId = VoxelId(2);

    fn state_of(block: VoxelId) -> BlockStateId {
        BlockStateId::checked(block.0, 3).expect("a block of the test table")
    }

    fn light_state() -> LightState {
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
        LightState::new(LightWorld::new(registry, LightBounds::new(0, 1)))
    }

    fn load(column: ColumnPos, priority: Priority) -> Vec<QueuedEdit> {
        (0..2)
            .map(|y| QueuedEdit {
                edit: Edit::LoadSection {
                    entity: 0,
                    pos: SectionPos::new(column.x, y, column.z),
                    blocks: Arc::new(VoxelPalette(PalettedContainer::Homogeneous(state_of(AIR)))),
                },
                priority,
            })
            .collect()
    }

    fn load_columns(state: &mut LightState, columns: i32) {
        let edits = (0..columns)
            .flat_map(|x| load(ColumnPos::new(x, 0), DEFAULT_PRIORITY))
            .collect();
        state.receive(ToLight::Edits(edits));
    }

    fn loaded(state: &LightState) -> usize {
        state.world.loaded_sections().count()
    }

    #[test]
    fn intake_is_bounded_per_round_and_loses_nothing() {
        let mut state = light_state();
        let limit = state.intake.columns_per_round;
        let columns = limit as i32 * 4 + 7;
        load_columns(&mut state, columns);

        state.intake();
        assert_eq!(loaded(&state), limit * 2);
        assert!(state.has_intake());

        let mut rounds = 1;
        while state.has_intake() {
            state.intake();
            rounds += 1;
            assert!(rounds < 100, "intake never drained");
        }
        assert_eq!(rounds, (columns as usize).div_ceil(limit));
        assert_eq!(loaded(&state), columns as usize * 2);
    }

    #[test]
    fn a_block_change_is_not_deferred_behind_a_bulk_load() {
        let mut state = light_state();
        let limit = state.intake.columns_per_round;
        let pos = BlockPos::new(8, 8, 8);
        load_columns(&mut state, 1);
        state.intake();

        load_columns(&mut state, limit as i32 * 3);
        state.receive(ToLight::Edits(vec![QueuedEdit {
            edit: Edit::SetBlock {
                pos,
                block: state_of(TORCH),
            },
            priority: DEFAULT_PRIORITY,
        }]));
        state.intake();

        assert!(state.has_intake());
        assert_eq!(state.world.block_at(pos), TORCH);
    }

    #[test]
    fn the_most_urgent_column_is_admitted_before_the_backlog() {
        let mut state = light_state();
        let limit = state.intake.columns_per_round;
        load_columns(&mut state, limit as i32 * 2);

        let urgent = ColumnPos::new(-1, 0);
        state.receive(ToLight::Edits(load(urgent, 0)));
        state.intake();

        assert!(
            state
                .world
                .section(SectionPos::new(urgent.x, 0, urgent.z))
                .is_some()
        );
        assert!(state.has_intake());
    }

    /// A seam is lit against whatever stands beside it, so a column is only
    /// finished once its neighbours are.
    #[test]
    fn a_column_is_not_settled_while_a_neighbour_still_has_lighting_work() {
        let column = ColumnPos::new(0, 0);
        let mut state = light_state();
        state.receive(ToLight::AwaitColumns(vec![column]));
        assert_eq!(
            state.take_settled(),
            [column],
            "nothing is owed to the column or its ring"
        );

        let mut state = light_state();
        state.receive(ToLight::Edits(load(ColumnPos::new(1, 0), DEFAULT_PRIORITY)));
        state.intake();
        state.receive(ToLight::AwaitColumns(vec![column]));
        assert!(
            state.take_settled().is_empty(),
            "the neighbour's work will repair this column's seam"
        );
    }
}
