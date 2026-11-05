//! The block simulation of a dimension world: each tick it runs the step of the block kernels
//! over the sections of the cell, with the rules of the blocks that the game supplies.
//!
//! The order of one tick is the order of vanilla. At the sync point, the halo takes the changes
//! of the owners, the ticks of the columns that landed join the queues, and the messages of the
//! other cells and then the block writes of the tick apply. Then the scheduled block ticks run,
//! then the scheduled fluid ticks, then the random ticks, then the block events. Neighbour and
//! shape updates run inside the phase that causes them. The block events that a block took go to
//! the clients that hold its column, and the items of a destroyed block go out as
//! [`ItemDropped`].

use std::mem;

use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Query, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Res, ResMut, SystemParam};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::{
    self, CellBlockWrite, HaloBlockChanges, VoxelUpdateSet,
};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{BlockTicking, EntityTicking};
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockShard, ChainLedger, ChainOrigin, Env, GameTick, KernelConfig, Message,
    MessageBody, PartitionKind, Phase, PhaseInput, RandomPicks, RandomTickTable, SavedTick,
    SectionProperties, SectionTicksRef, ShardId, SimulatedSections, StepAt, StepOutput, StepReport,
    StreamSeed, SubTick, TickKind, TickQueue, VanillaOrder,
};
use mcrs_minecraft_volume::{Access, Volume};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashSet;

use crate::block_drops::{self, BlockDrops, ItemDropped};
use crate::block_events;
use crate::cell_mail::CellOutbox;
use crate::cell_volume::CellVolume;
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;
use crate::column_ticks::{ColumnTicks, LandedTicks, StoredTick};
use crate::coordinator::clock::GameTime;
use crate::random_tick_speed::RandomTickSpeed;
use crate::world_kinds::{WorldFacts, WorldKinds};
use crate::world_seed::WorldSeed;

/// Rules under which no block reacts: no block has a behaviour that a specification states yet.
/// Writes, neighbour and shape updates, scheduled ticks and block events still run, and a tick
/// or an event reaches its block with no effect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InertBlocks;

impl BlockBehaviour<WorldKinds> for InertBlocks {}

impl BlockDrops for InertBlocks {}

/// The rules of the blocks of a dimension world.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct BlockRules<B>(pub B);

/// Which block rules the host builds into each dimension world: a function that adds a
/// [`BlockSimulationPlugin`] to the world, which holds the registries of the host by then. Insert
/// it into the host before the dimensions spawn; without it, the dimensions run [`InertBlocks`].
#[derive(Resource, Clone, Copy, Debug)]
pub struct DimensionBlockRules {
    install: fn(&mut SubApp),
}

impl DimensionBlockRules {
    #[must_use]
    pub fn new(install: fn(&mut SubApp)) -> Self {
        Self { install }
    }

    pub fn install(&self, sub_app: &mut SubApp) {
        (self.install)(sub_app);
    }
}

impl Default for DimensionBlockRules {
    fn default() -> Self {
        Self::new(|sub_app| {
            sub_app.add_plugins(BlockSimulationPlugin::new(InertBlocks));
        })
    }
}

/// The block simulation of a dimension world under the rules `B`: each [`BlockSetRequest`] is a
/// set of the kernel, with all its flags.
pub struct BlockSimulationPlugin<B> {
    rules: B,
}

impl<B> BlockSimulationPlugin<B> {
    #[must_use]
    pub fn new(rules: B) -> Self {
        Self { rules }
    }
}

impl<B: BlockBehaviour<WorldKinds> + BlockDrops + Clone + Send + 'static> Plugin
    for BlockSimulationPlugin<B>
{
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            VoxelUpdateSet::ApplyChanges.in_set(TickSet::WorldWrite),
        );
        app.configure_sets(FixedPostUpdate, VoxelUpdateSet::NetworkSync);
        app.insert_resource(BlockRules(self.rules.clone()));
        app.init_resource::<BlockSimulation>();
        app.init_resource::<LandedTicks>();
        app.add_message::<ColumnChange>();
        app.add_message::<ItemDropped>();
        app.add_systems(
            FixedUpdate,
            run_block_simulation::<B>.in_set(VoxelUpdateSet::ApplyChanges),
        );
    }
}

/// A message of the block kernels from one cell of a world to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationMail {
    /// A write, a tick, an event or an update for a section of the cell that receives it.
    Message(Message<WorldKinds>),
    /// How far a chain that sent updates got, for the cells that continue it.
    Ledger(ChainLedger),
}

impl SimulationMail {
    /// The column whose owner takes the message, or `None` for a ledger, which every other
    /// cell takes.
    #[must_use]
    pub fn column(&self) -> Option<ColumnPos> {
        match self {
            Self::Message(message) => Some(ColumnPos::from(message.section())),
            Self::Ledger(_) => None,
        }
    }
}

/// Whether the ticks of a column changed since the column landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickChange {
    /// The column holds only the saved ticks it landed with, none of them activated.
    Unchanged,
    /// A saved tick activated, or the column holds a tick scheduled since it landed.
    Changed,
}

/// The ticks that a column held when it went cold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenTicks {
    pub ticks: ColumnTicks,
    pub change: TickChange,
}

/// The queues of the block kernels of a dimension world, and what crosses to the other cells.
#[derive(Resource, Default)]
pub struct BlockSimulation {
    shard: BlockShard<WorldKinds>,
    config: KernelConfig,
    out: StepOutput<WorldKinds>,
    /// The messages of the other cells for the next step, in the order of the cells.
    inbound: Vec<Message<WorldKinds>>,
    ledgers: Vec<ChainLedger>,
    /// The sections with saved ticks that wait for their section to run ticks.
    held: FxHashSet<SectionPos>,
    /// The columns in which a saved tick activated since they landed.
    activated: FxHashSet<ColumnPos>,
    /// The random ticks of each block state under the rules, once the block table landed.
    random_table: Option<RandomTickTable>,
    random_seed: Option<StreamSeed>,
    random_sections: Vec<SectionPos>,
    drops: Vec<ItemStack>,
}

impl BlockSimulation {
    pub(crate) fn receive(&mut self, mail: SimulationMail) {
        match mail {
            SimulationMail::Message(message) => self.inbound.push(message),
            SimulationMail::Ledger(ledger) => self.ledgers.push(ledger),
        }
    }

    /// Holds the saved ticks of a column that landed. The k-th tick of a list of m ticks gets the
    /// order k - m, so the ticks keep their saved order and run before the ticks of the tick that
    /// are due at the same time. A later copy of a held or queued tick is dropped.
    fn hold(&mut self, ticks: &ColumnTicks) {
        hold_list(self.shard.block_ticks_mut(), &mut self.held, &ticks.block);
        hold_list(self.shard.fluid_ticks_mut(), &mut self.held, &ticks.fluid);
    }

    /// Activates the saved ticks of each section that runs ticks now: a tick that was saved with
    /// the delay `t` is due `t` ticks after now.
    fn activate<V: SimulatedSections>(&mut self, volume: &V, now: GameTick) {
        let Self {
            shard,
            held,
            activated,
            ..
        } = self;
        held.retain(|section| {
            let runs = volume
                .properties(*section)
                .contains(SectionProperties::RUNS_SCHEDULED_TICKS);
            if runs {
                shard.block_ticks_mut().activate(*section, now);
                shard.fluid_ticks_mut().activate(*section, now);
                activated.insert(ColumnPos::from(*section));
            }
            !runs
        });
    }

    /// Takes the ticks of the sections `rows` of `column` out of the queues, as the data layer
    /// keeps them: first the saved ticks that did not activate, in their saved order, then the
    /// active ticks, in the order they run when due together, each with its delay from `now`.
    pub fn take_column(
        &mut self,
        column: ColumnPos,
        rows: impl IntoIterator<Item = i32>,
        now: GameTick,
    ) -> TakenTicks {
        let mut block = Taken::default();
        let mut fluid = Taken::default();
        for y in rows {
            let section = SectionPos::new(column.x, y, column.z);
            self.held.remove(&section);
            if let Some(ticks) = self.shard.block_ticks_mut().take_section(section) {
                block.add(ticks.as_ref(), now);
            }
            if let Some(ticks) = self.shard.fluid_ticks_mut().take_section(section) {
                fluid.add(ticks.as_ref(), now);
            }
        }
        let activated = self.activated.remove(&column);
        let change = if activated || !block.active.is_empty() || !fluid.active.is_empty() {
            TickChange::Changed
        } else {
            TickChange::Unchanged
        };
        TakenTicks {
            ticks: ColumnTicks {
                block: block.into_list(),
                fluid: fluid.into_list(),
            },
            change,
        }
    }
}

fn hold_list<T: TickKind>(
    queue: &mut TickQueue<T>,
    held: &mut FxHashSet<SectionPos>,
    list: &[StoredTick<T>],
) {
    let Ok(count) = u32::try_from(list.len()) else {
        return;
    };
    for (index, tick) in (0..count).zip(list) {
        let Some(sequence) = SubTick::restored(index, count) else {
            continue;
        };
        if queue.contains(tick.pos, tick.kind) {
            continue;
        }
        queue.hold_saved(SavedTick {
            pos: tick.pos,
            kind: tick.kind,
            delay: tick.delay,
            priority: tick.priority,
            sequence,
        });
        held.insert(SectionPos::from(tick.pos));
    }
}

struct Ordered<T> {
    sequence: SubTick,
    tick: StoredTick<T>,
}

struct Taken<T> {
    held: Vec<Ordered<T>>,
    active: Vec<Ordered<T>>,
}

impl<T> Default for Taken<T> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            active: Vec::new(),
        }
    }
}

impl<T: TickKind> Taken<T> {
    fn add(&mut self, ticks: SectionTicksRef<'_, T>, now: GameTick) {
        self.held.extend(ticks.held().map(|tick| Ordered {
            sequence: tick.sequence,
            tick: StoredTick {
                pos: tick.pos,
                kind: tick.kind,
                delay: tick.delay,
                priority: tick.priority,
            },
        }));
        self.active.extend(ticks.active().map(|tick| {
            let delay = tick.key.due.0.saturating_sub(now.0);
            Ordered {
                sequence: tick.key.sequence,
                tick: StoredTick {
                    pos: tick.pos,
                    kind: tick.kind,
                    delay: i32::try_from(delay).unwrap_or(if delay < 0 {
                        i32::MIN
                    } else {
                        i32::MAX
                    }),
                    priority: tick.key.priority,
                },
            }
        }));
    }

    fn into_list(mut self) -> Vec<StoredTick<T>> {
        self.held.sort_by_key(|ordered| ordered.sequence);
        self.active.sort_by_key(|ordered| ordered.sequence);
        self.held
            .into_iter()
            .chain(self.active)
            .map(|ordered| ordered.tick)
            .collect()
    }
}

/// What the cells of a world hand each other besides the blocks.
#[derive(SystemParam)]
struct CellLinks<'w> {
    owned: Option<Res<'w, OwnedColumns>>,
    outbox: Option<ResMut<'w, CellOutbox>>,
    halo: Option<ResMut<'w, HaloBlockChanges>>,
}

#[derive(SystemParam)]
struct Kernel<'w, B: BlockBehaviour<WorldKinds> + Send + 'static> {
    simulation: ResMut<'w, BlockSimulation>,
    landed: ResMut<'w, LandedTicks>,
    rules: Res<'w, BlockRules<B>>,
    definitions: Option<Res<'w, Blocks>>,
    time: Option<Res<'w, GameTime>>,
}

/// The sections that take random ticks, and what their picks draw from.
#[derive(SystemParam)]
struct RandomTicking<'w, 's> {
    sections: Query<'w, 's, &'static SectionPos, With<EntityTicking>>,
    speed: Option<Res<'w, RandomTickSpeed>>,
    seed: Option<Res<'w, WorldSeed>>,
}

/// Where the outcome of a step goes besides the blocks.
#[derive(SystemParam)]
struct StepOutlets<'w, 's> {
    streams: Query<'w, 's, &'static ColumnStreams>,
    changes: MessageWriter<'w, ColumnChange>,
    dropped: MessageWriter<'w, ItemDropped>,
}

#[expect(
    clippy::too_many_lines,
    reason = "the sync point, the four phases and the hand-off of their output are one ordered step"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the step reads or writes"
)]
fn run_block_simulation<B: BlockBehaviour<WorldKinds> + BlockDrops + Send + 'static>(
    mut requests: MessageReader<BlockSetRequest>,
    mut blocks: WorldViewMut,
    mut placed: MessageWriter<BlockPlaced>,
    ticking: Query<(), With<BlockTicking>>,
    dims: Query<(Entity, Option<&ResourceKeyBuf<DimensionKey>>), With<Dimension>>,
    mut links: CellLinks,
    mut kernel: Kernel<B>,
    random: RandomTicking,
    mut outlets: StepOutlets,
) {
    if let Some(halo) = links.halo.as_mut() {
        voxel_update::apply_halo_changes(&mut blocks, &mut placed, &mut halo.0);
    }
    let owned = links.owned.as_deref();
    let Ok((dimension, dimension_key)) = dims.single() else {
        requests.clear();
        return;
    };
    let Some(definitions) = kernel.definitions.as_deref() else {
        // The block table lands with the registries; until then a write is stored as it is.
        for request in requests.read() {
            voxel_update::apply_write(
                &mut blocks,
                &mut placed,
                request.dimension,
                cell_write(request),
            );
        }
        return;
    };
    let now = GameTick(kernel.time.as_deref().map_or(0, |time| time.0));
    let partition = ShardId(owned.map_or(0, |owned| owned.index().0));
    let simulation = &mut *kernel.simulation;
    let mut inbound = mem::take(&mut simulation.inbound);
    let ledgers = mem::take(&mut simulation.ledgers);
    let mut out = mem::take(&mut simulation.out);

    let mut counter: u32 = 0;
    for request in requests.read() {
        if request.dimension != dimension {
            tracing::warn!(
                target: "voxel_update",
                dimension = ?request.dimension,
                pos = ?request.pos,
                "a block write names another dimension; dropping",
            );
            continue;
        }
        if !owned_columns::owns(owned, ColumnPos::from(request.pos)) {
            if let Some(outbox) = links.outbox.as_mut() {
                outbox.0.writes.push(cell_write(request));
            }
            continue;
        }
        let stamp = SubTick::runtime(now, counter);
        counter = counter.saturating_add(1);
        inbound.push(Message {
            stamp,
            origin: ChainOrigin { stamp, partition },
            body: MessageBody::Set {
                pos: request.pos,
                state: request.new_state,
                flags: request.flags,
                limit: u16::try_from(request.recursion_left).unwrap_or(0),
            },
        });
    }

    let Some(view) = blocks.get_mut(dimension) else {
        return;
    };
    let mut volume = CellVolume {
        view,
        owned,
        ticking: &ticking,
    };
    for landed in kernel.landed.0.drain(..) {
        if owned_columns::owns(owned, landed.column) {
            simulation.hold(&landed.ticks);
        }
    }
    simulation.activate(&volume, now);
    // A message for a column of the halo would make the step refuse the whole batch; the
    // coordinator hands each message to its owner, so this drops nothing it routed.
    inbound.retain(|message| {
        !matches!(
            volume.access(message.section()),
            Access::Read | Access::Foreign
        )
    });

    let rules = &kernel.rules.0;
    let table = simulation.random_table.get_or_insert_with(|| {
        RandomTickTable::new(definitions.states(), |state| rules.random_ticks(state))
    });
    let speed = random.speed.as_deref().copied().unwrap_or_default().0;
    let mut sections = mem::take(&mut simulation.random_sections);
    if speed > 0 && !table.is_empty() {
        sections.extend(random.sections.iter().copied());
        // The picks of a section do not depend on the order; what one pick sets for the
        // picks of a later section does.
        sections.sort_unstable_by_key(|section| (section.x, section.z, section.y));
    }
    let seed = *simulation.random_seed.get_or_insert_with(|| {
        let world_seed = random.seed.as_deref().map_or(0, |seed| seed.0);
        let key = dimension_key.map_or("", |key| key.as_str());
        StreamSeed::new(world_seed as i64, key)
    });

    let facts = WorldFacts::new(definitions);
    let config = simulation.config;
    let env = Env {
        partition,
        facts: &facts,
        behaviour: rules,
        policy: &VanillaOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let phases = [
        (Phase::BlockTicks, inbound.as_slice(), ledgers.as_slice()),
        (Phase::FluidTicks, [].as_slice(), [].as_slice()),
        (Phase::BlockEvents, [].as_slice(), [].as_slice()),
    ];
    for (phase, messages, chains) in phases {
        if phase == Phase::BlockEvents {
            let picks = RandomPicks {
                sections: &sections,
                speed,
                table,
                seed,
            };
            mcrs_minecraft_simulation::random_step(
                &mut simulation.shard,
                now,
                &mut volume,
                &env,
                picks,
                &mut out,
            );
        }
        let input = PhaseInput {
            inbound: messages,
            ledgers: chains,
        };
        let at = StepAt { now, phase };
        if let Err(error) = mcrs_minecraft_simulation::step(
            &mut simulation.shard,
            at,
            &mut volume,
            &env,
            input,
            &mut out,
        ) {
            tracing::error!(%error, ?phase, "the block step refused its input");
        }
    }
    sections.clear();
    simulation.random_sections = sections;

    for write in out.writes.drain(..) {
        let chunk_pos = SectionPos::from(write.pos);
        let Some(chunk) = volume.view.section_entity(chunk_pos) else {
            continue;
        };
        placed.write(BlockPlaced {
            chunk,
            chunk_pos,
            block_pos: write.pos,
            old_state: write.old,
            new_state: write.new,
            flags: write.flags,
        });
    }
    if !out.delivered.is_empty()
        && let Ok(streams) = outlets.streams.get(dimension)
    {
        block_events::publish_block_events(&out.delivered, streams, &mut outlets.changes);
    }
    block_drops::drop_items(
        rules,
        &out.destroyed,
        &mut simulation.drops,
        &mut outlets.dropped,
    );
    if let Some(outbox) = links.outbox.as_mut() {
        let mail = &mut outbox.0.simulation;
        mail.extend(out.outbound.drain(..).map(SimulationMail::Message));
        mail.extend(out.chains.drain(..).map(SimulationMail::Ledger));
    }
    out.outbound.clear();
    out.chains.clear();
    out.marked.clear();
    out.delivered.clear();
    out.destroyed.clear();
    out.report = StepReport::default();
    simulation.out = out;
    inbound.clear();
    simulation.inbound = inbound;
}

fn cell_write(request: &BlockSetRequest) -> CellBlockWrite {
    CellBlockWrite {
        pos: request.pos,
        new_state: request.new_state,
        flags: request.flags,
        recursion_left: request.recursion_left,
    }
}
