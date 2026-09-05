use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use mcrs_voxel_math::{ChunkPos, ColumnPos};

use crate::block::LightRegistry;
use crate::epoch::LightUpdate;
use crate::level::LightBounds;
use crate::queue::{DEFAULT_PRIORITY, LightQueue, Priority};
use crate::region::BlockBox;
use crate::world::{Edit, LightWorld};
use crate::{BlockLight, SkyLight};

/// Owns the block and light data. The ECS holds only what has been published.
#[derive(Resource)]
pub struct Lighting(pub LightWorld);

/// Edits waiting to be applied. Push from any system ordered before
/// [`LightSet::Intake`], which is where they reach the world and where the
/// lighting work they create joins [`LightWorkQueue`]. The block change itself
/// is never deferred — only the lighting is.
#[derive(Resource, Default)]
pub struct PendingEdits {
    columns: HashMap<ColumnPos, ColumnEdits>,
    /// The same columns again, keyed so that the most urgent sorts first.
    order: BTreeSet<(Priority, ColumnPos)>,
    /// Columns no budget holds back, in the order they became urgent.
    immediate: Vec<ColumnPos>,
}

struct ColumnEdits {
    edits: Vec<Edit>,
    priority: Priority,
    immediate: bool,
}

impl PendingEdits {
    pub fn push(&mut self, edit: Edit) {
        self.push_with_priority(edit, DEFAULT_PRIORITY);
    }

    /// Lower is more urgent. A column keeps the most urgent priority it was
    /// given while it waits.
    pub fn push_with_priority(&mut self, edit: Edit, priority: Priority) {
        let column = edit.column();
        // A block change takes effect the tick it is pushed; a section load
        // held back past one in the same column would then overwrite it.
        let immediate = matches!(edit, Edit::SetBlock { .. });
        match self.columns.get_mut(&column) {
            Some(waiting) => {
                if priority < waiting.priority {
                    self.order.remove(&(waiting.priority, column));
                    waiting.priority = priority;
                    self.order.insert((priority, column));
                }
                if immediate && !waiting.immediate {
                    waiting.immediate = true;
                    self.immediate.push(column);
                }
                waiting.edits.push(edit);
            }
            None => {
                self.order.insert((priority, column));
                if immediate {
                    self.immediate.push(column);
                }
                self.columns.insert(
                    column,
                    ColumnEdits {
                        edits: vec![edit],
                        priority,
                        immediate,
                    },
                );
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Whether this column has edits that have not reached the world yet.
    pub fn holds(&self, column: ColumnPos) -> bool {
        self.columns.contains_key(&column)
    }

    /// The most urgent `limit` columns plus every column holding a block
    /// change, each taken whole.
    ///
    /// Costs what it hands out, never what is waiting: a backlog of thousands
    /// of columns is not re-read to admit fifty.
    fn take_admitted(&mut self, limit: usize) -> Vec<(Priority, Vec<Edit>)> {
        let mut admitted = std::mem::take(&mut self.immediate);
        admitted.extend(
            self.order
                .iter()
                .map(|&(_, column)| column)
                .filter(|column| !self.columns[column].immediate)
                .take(limit),
        );
        admitted
            .into_iter()
            .map(|column| {
                let waiting = self
                    .columns
                    .remove(&column)
                    .expect("an admitted column is waiting");
                self.order.remove(&(waiting.priority, column));
                (waiting.priority, waiting.edits)
            })
            .collect()
    }
}

/// Lighting work that has been applied to the world but not yet computed.
///
/// Exposed so the game can re-order it as the player moves, the way vanilla
/// re-sorts chunk tasks when a ticket level changes.
#[derive(Resource, Default)]
pub struct LightWorkQueue(pub LightQueue);

/// How much lighting work may be under way at once.
///
/// The point is not to finish sooner but to keep any single pass bounded: light
/// is allowed to lag, a pass that never ends is not.
#[derive(Resource, Copy, Clone, Debug)]
pub struct LightBudget {
    /// Cells of working field one epoch may cover. Counting influences instead
    /// bounds nothing: the batch is unioned into a single field and the cost of
    /// an epoch is that field's volume, whether two influences or two thousand
    /// spanned it.
    pub cells_per_epoch: u64,
    /// Epochs allowed in flight together.
    pub epochs_in_flight: usize,
}

impl Default for LightBudget {
    fn default() -> Self {
        Self {
            // About a dozen columns of a full-height world once the fifteen
            // cell influence radius and section rounding are paid for on the
            // perimeter: a few milliseconds of work, and four times the value
            // per column of lighting one on its own.
            cells_per_epoch: 4 << 20,
            epochs_in_flight: std::thread::available_parallelism().map_or(1, |n| n.get()),
        }
    }
}

/// How many section columns one tick may take in.
///
/// Loading a column costs a 256 block column sky rescan: 34us measured over a
/// 137x137 bulk load. Only section loads and unloads are held back — a block
/// change takes effect the tick it is pushed, and there are never thousands of
/// those in one tick.
#[derive(Resource, Copy, Clone, Debug)]
pub struct IntakeBudget {
    pub columns_per_tick: usize,
}

impl Default for IntakeBudget {
    fn default() -> Self {
        // 2ms of a 50ms tick at the measured 34us per column. The ceiling that
        // matters is the 15s keep-alive window, which this misses by four
        // orders of magnitude however many columns finish loading at once.
        Self {
            columns_per_tick: 58,
        }
    }
}

/// How much the sky is dimmed right now: 0 at noon, 11 at midnight.
#[derive(Resource, Default, Copy, Clone, PartialEq, Eq, Debug)]
pub struct SkyDarken(pub u8);

/// Maps section positions to entities and back.
///
/// The reverse direction is not a convenience. When a section entity goes away
/// the component is already gone, so its position cannot be recovered from
/// anything but this map.
#[derive(Resource, Default)]
pub struct SectionIndex {
    by_position: HashMap<ChunkPos, Entity>,
    by_entity: HashMap<Entity, ChunkPos>,
}

impl SectionIndex {
    pub fn entity(&self, pos: ChunkPos) -> Option<Entity> {
        self.by_position.get(&pos).copied()
    }

    pub fn position(&self, entity: Entity) -> Option<ChunkPos> {
        self.by_entity.get(&entity).copied()
    }

    fn insert(&mut self, pos: ChunkPos, entity: Entity) {
        self.by_position.insert(pos, entity);
        self.by_entity.insert(entity, pos);
    }

    fn remove(&mut self, entity: Entity) -> Option<ChunkPos> {
        let pos = self.by_entity.remove(&entity)?;
        // A section despawned and respawned in the same tick reaches this in one
        // batch, and the new entity is already the occupant of that position.
        if self.by_position.get(&pos) == Some(&entity) {
            self.by_position.remove(&pos);
        }
        Some(pos)
    }
}

/// The epochs in flight.
///
/// This is also the answer to "has the light settled". Systems whose behaviour
/// has to be reproducible — mob spawning, crop growth, saving a region — should
/// run only when it has, because light published a tick later than an edit is
/// fine for rendering and not fine for simulation.
#[derive(Resource, Default)]
pub struct LightEpoch(Vec<InFlight>);

struct InFlight {
    area: BlockBox,
    task: Task<LightUpdate>,
}

impl LightEpoch {
    pub fn is_running(&self) -> bool {
        !self.0.is_empty()
    }

    /// Whether an epoch under way may still write into `area`.
    pub fn touches(&self, area: BlockBox) -> bool {
        self.0.iter().any(|epoch| epoch.area.intersects(area))
    }
}

/// Run condition for systems that must see settled light.
pub fn light_has_settled(
    epoch: Res<LightEpoch>,
    pending: Res<PendingEdits>,
    queue: Res<LightWorkQueue>,
) -> bool {
    !epoch.is_running() && pending.is_empty() && queue.0.is_empty()
}

/// The tick-loop steps, in the order they must run.
///
/// Resource conflicts alone would serialise them but leave the order to the
/// scheduler, and most of the orders that leaves open are wrong: dispatching
/// before edits are collected loses a tick, and dispatching before publishing
/// throws away a finished epoch.
#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub enum LightSet {
    /// Section entities are indexed and their removal turned into edits.
    Track,
    /// A finished epoch is written into [`BlockLight`] and [`SkyLight`].
    Publish,
    /// Edits are applied to the world and their lighting work is queued.
    Intake,
    /// The most urgent queued work, up to the budget, is handed to a worker.
    Dispatch,
}

pub struct LightPlugin {
    pub registry: Arc<LightRegistry>,
    pub bounds: LightBounds,
    /// Clear for a dimension with no sky, so the sky layer never gains a source.
    pub sky: bool,
}

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        let world = LightWorld::new(Arc::clone(&self.registry), self.bounds);
        app.insert_resource(Lighting(if self.sky { world } else { world.without_sky() }))
            .init_resource::<PendingEdits>()
            .init_resource::<LightWorkQueue>()
            .init_resource::<LightBudget>()
            .init_resource::<IntakeBudget>()
            .init_resource::<SectionIndex>()
            .init_resource::<SkyDarken>()
            .init_resource::<LightEpoch>()
            .add_systems(
                Last,
                (
                    track_sections.in_set(LightSet::Track),
                    publish_light.in_set(LightSet::Publish),
                    intake_edits.in_set(LightSet::Intake),
                    dispatch_epoch.in_set(LightSet::Dispatch),
                )
                    .chain(),
            );
    }
}

fn track_sections(
    mut index: ResMut<SectionIndex>,
    mut pending: ResMut<PendingEdits>,
    added: Query<(Entity, &ChunkPos), Added<ChunkPos>>,
    mut removed: RemovedComponents<ChunkPos>,
) {
    for (entity, pos) in &added {
        index.insert(*pos, entity);
    }
    for entity in removed.read() {
        if let Some(pos) = index.remove(entity) {
            pending.push(Edit::UnloadSection { pos });
        }
    }
}

fn publish_light(
    mut lighting: ResMut<Lighting>,
    mut running: ResMut<LightEpoch>,
    index: Res<SectionIndex>,
    mut commands: Commands,
    mut lit: Query<(&mut BlockLight, &mut SkyLight)>,
) {
    let mut finished = Vec::new();
    running
        .0
        .retain_mut(|epoch| match block_on(poll_once(&mut epoch.task)) {
            Some(update) => {
                finished.push(update);
                false
            }
            None => true,
        });

    let published: Vec<ChunkPos> = finished
        .into_iter()
        .flat_map(|update| lighting.0.apply(update))
        .collect();

    for pos in published {
        let Some(entity) = index.entity(pos) else {
            continue;
        };
        let Some(section) = lighting.0.section(pos) else {
            continue;
        };
        let block = BlockLight(section.block_light.clone());
        let sky = SkyLight(section.sky_light.clone());
        match lit.get_mut(entity) {
            // Only a real change should wake whatever rebuilds meshes.
            Ok((mut existing_block, mut existing_sky)) => {
                existing_block.set_if_neq(block);
                existing_sky.set_if_neq(sky);
            }
            Err(_) => {
                if let Ok(mut entity) = commands.get_entity(entity) {
                    entity.insert((block, sky));
                }
            }
        }
    }
}

fn intake_edits(
    mut lighting: ResMut<Lighting>,
    mut pending: ResMut<PendingEdits>,
    mut queue: ResMut<LightWorkQueue>,
    budget: Res<IntakeBudget>,
) {
    if pending.is_empty() {
        return;
    }
    // A column's edits go in one call so that a stack of section loads rescans
    // its 256 sky columns once rather than once per section — which is why the
    // budget counts columns and never splits one across ticks.
    for (priority, edits) in pending.take_admitted(budget.columns_per_tick) {
        for (column, influence) in lighting.0.apply_edits(edits) {
            queue.0.push_with_priority(column, influence, priority);
        }
    }
}

fn dispatch_epoch(
    mut lighting: ResMut<Lighting>,
    mut queue: ResMut<LightWorkQueue>,
    mut running: ResMut<LightEpoch>,
    budget: Res<LightBudget>,
) {
    if queue.0.is_empty() {
        return;
    }
    // Without a task pool there is no worker to hand the batch to, so the queue
    // keeps it and the light simply lags until one exists.
    let Some(pool) = AsyncComputeTaskPool::try_get() else {
        return;
    };
    let free = budget.epochs_in_flight.saturating_sub(running.0.len());
    if free == 0 {
        return;
    }
    let occupied: Vec<BlockBox> = running.0.iter().map(|epoch| epoch.area).collect();
    for batch in queue
        .0
        .drain_batches(budget.cells_per_epoch, &occupied, free)
    {
        let Some(job) = lighting.0.prepare_batch(batch) else {
            continue;
        };
        let area = job.area();
        running.0.push(InFlight {
            area,
            task: pool.spawn(async move { job.run() }),
        });
    }
}
