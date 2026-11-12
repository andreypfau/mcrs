//! The block simulation of a dimension world: each tick it runs the step of the block kernels
//! over the sections of the cell, with the rules of the blocks that the game supplies.
//!
//! The order of one tick is the order of vanilla. At the sync point, the ticks of the columns
//! that landed join the queues, and the messages of the other cells and then the block writes of
//! the tick apply. Then the scheduled block ticks run,
//! then the scheduled fluid ticks, then the random ticks, then the broadcast of the marked
//! blocks, then the block events. Neighbour and shape updates run inside the phase that causes
//! them. The block events that a block took go to the clients near it after the broadcast, so the
//! changes that the events cause go out in the broadcast of the next tick. The items of a
//! destroyed block go out as [`ItemDropped`].

use std::mem;

use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Has, Query, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::common_conditions::{not, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy_ecs::system::{Res, ResMut, SystemParam};
use bevy_ecs::world::World;
use bevy_tasks::{ComputeTaskPool, ParallelSliceMut, TaskPool};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::block_update::{
    BlockDestroyRequest, BlockPlaced, BlockSetRequest,
};
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::{self, CellBlockWrite, VoxelUpdateSet};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{BlockTicking, EntityTicking};
use mcrs_minecraft_engine_world::world::storage::random_ticks::{
    RandomListing, RandomTickIndex, RandomTickIndexPlugin,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEvent;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockShard, BlockUpdateFlags, BlockWrite, ChainLedger, ChainOrigin,
    Env, Footprints, GameTick, KernelConfig, Message, MessageBody, PartitionKind, Phase,
    PhaseInput, RandomPicks, RandomTickTable, SavedTick, SectionProperties, SectionTicksRef,
    Serial, ShardId, SimulatedSections, Spread, StepAt, StepOutput, StreamSeed, SubTick, TickKind,
    TickQueue, VanillaOrder,
};
use mcrs_minecraft_volume::PalettedSection;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashSet;

use crate::block_update::{BroadcastMarks, remove_replaced_block_entities};
use crate::bus::PacketPayload;
use crate::cell_mail::CellOutbox;
use crate::cell_volume::CellVolume;
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;
use crate::column_ticks::{ColumnTicks, LandedTicks, StoredTick};
use crate::coordinator::clock::GameTime;
use crate::random_tick_speed::RandomTickSpeed;
use crate::world_kinds::{WorldFacts, WorldKinds};
use mcrs_minecraft_engine_world::block_drops::{BlockDrops, ExperienceDropped, ItemDropped};
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_voxel::BlockPos;

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
pub struct DimensionBlockRules(pub fn(&mut SubApp));

impl Default for DimensionBlockRules {
    fn default() -> Self {
        Self(|sub_app| {
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
        app.insert_resource(BlockRules(self.rules.clone()));
        app.init_resource::<BlockSimulation>();
        app.init_resource::<LandedTicks>();
        app.init_resource::<BroadcastMarks>();
        app.add_message::<ItemDropped>();
        app.add_message::<ExperienceDropped>();
        app.add_message::<BlockDestroyRequest>();
        app.add_plugins(RandomTickIndexPlugin);
        app.add_systems(
            FixedUpdate,
            (
                index_random_ticks::<B>.run_if(
                    resource_exists::<Blocks>.and_then(not(resource_exists::<RandomTickIndex>)),
                ),
                run_block_simulation::<B>,
            )
                .chain()
                .in_set(TickSet::WorldWrite),
        );
        app.add_systems(
            FixedPostUpdate,
            remove_replaced_block_entities.in_set(VoxelUpdateSet::NetworkSync),
        );
    }
}

/// The ticks that a column held when it went cold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenTicks {
    pub ticks: ColumnTicks,
    /// A saved tick activated, or the column holds a tick scheduled since it landed.
    pub changed: bool,
}

/// The queues of the block kernels of a dimension world, and what crosses to the other cells.
#[derive(Resource, Default)]
pub struct BlockSimulation {
    shard: BlockShard<WorldKinds>,
    out: StepOutput<WorldKinds>,
    /// The messages of the other cells for the next step, in the order of the cells.
    inbound: Vec<Message<WorldKinds>>,
    ledgers: Vec<ChainLedger>,
    /// The sections with saved ticks that wait for their section to run ticks.
    held: FxHashSet<SectionPos>,
    /// The columns in which a saved tick activated since they landed.
    activated: FxHashSet<ColumnPos>,
    /// The random ticks of each block state under the rules, once the block table landed: the
    /// table of the [`RandomTickIndex`].
    random_table: Option<RandomTickTable>,
    footprints: Footprints,
    random_seed: Option<StreamSeed>,
    random_listing: RandomListing,
    drops: Vec<ItemStack>,
}

impl BlockSimulation {
    pub(crate) fn receive(
        &mut self,
        messages: &mut Vec<Message<WorldKinds>>,
        ledgers: &mut Vec<ChainLedger>,
    ) {
        self.inbound.append(messages);
        self.ledgers.append(ledgers);
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
        let changed = activated || block.has_active() || fluid.has_active();
        TakenTicks {
            ticks: ColumnTicks {
                block: block.into_list(),
                fluid: fluid.into_list(),
            },
            changed,
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
    active: bool,
    sequence: SubTick,
    tick: StoredTick<T>,
}

struct Taken<T>(Vec<Ordered<T>>);

impl<T> Default for Taken<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: TickKind> Taken<T> {
    fn add(&mut self, ticks: SectionTicksRef<'_, T>, now: GameTick) {
        self.0.extend(ticks.held().map(|tick| Ordered {
            active: false,
            sequence: tick.sequence,
            tick: StoredTick {
                pos: tick.pos,
                kind: tick.kind,
                delay: tick.delay,
                priority: tick.priority,
            },
        }));
        self.0.extend(ticks.active().map(|tick| {
            let delay = tick.key.due.0.saturating_sub(now.0);
            Ordered {
                active: true,
                sequence: tick.key.sequence,
                tick: StoredTick {
                    pos: tick.pos,
                    kind: tick.kind,
                    delay: delay.clamp(i32::MIN.into(), i32::MAX.into()) as i32,
                    priority: tick.key.priority,
                },
            }
        }));
    }

    fn has_active(&self) -> bool {
        self.0.iter().any(|ordered| ordered.active)
    }

    fn into_list(mut self) -> Vec<StoredTick<T>> {
        self.0
            .sort_by_key(|ordered| (ordered.active, ordered.sequence));
        self.0.into_iter().map(|ordered| ordered.tick).collect()
    }
}

/// What the cells of a world hand each other besides the blocks.
#[derive(SystemParam)]
struct CellLinks<'w> {
    owned: Option<Res<'w, OwnedColumns>>,
    outbox: Option<ResMut<'w, CellOutbox>>,
}

#[derive(SystemParam)]
struct Kernel<'w, B: BlockBehaviour<WorldKinds> + Send + 'static> {
    simulation: ResMut<'w, BlockSimulation>,
    landed: ResMut<'w, LandedTicks>,
    rules: Res<'w, BlockRules<B>>,
    definitions: Option<Res<'w, Blocks>>,
    time: Res<'w, GameTime>,
}

/// What the random picks draw from.
#[derive(SystemParam)]
struct RandomTicking<'w> {
    speed: Res<'w, RandomTickSpeed>,
    seed: Option<Res<'w, WorldSeed>>,
}

/// Where the outcome of a step goes besides the blocks.
#[derive(SystemParam)]
struct StepOutlets<'w, 's> {
    marks: ResMut<'w, BroadcastMarks>,
    streams: Query<'w, 's, &'static ColumnStreams>,
    changes: MessageWriter<'w, ColumnChange>,
    dropped: MessageWriter<'w, ItemDropped>,
    experience: MessageWriter<'w, ExperienceDropped>,
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
    mut destroys: MessageReader<BlockDestroyRequest>,
    mut blocks: WorldViewMut,
    mut placed: MessageWriter<BlockPlaced>,
    ticking: Query<(), With<BlockTicking>>,
    dims: Query<(Entity, Option<&ResourceKeyBuf<DimensionKey>>), With<Dimension>>,
    mut links: CellLinks,
    mut kernel: Kernel<B>,
    random: RandomTicking,
    mut outlets: StepOutlets,
) {
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
            if request.flags.contains(BlockUpdateFlags::CLIENTS) {
                outlets.marks.mark(request.pos);
            }
        }
        if let Some(view) = blocks.get_mut(dimension) {
            let streams = outlets.streams.get(dimension).ok();
            outlets
                .marks
                .broadcast(&view, streams, &mut outlets.changes);
        }
        return;
    };
    let now = GameTick(kernel.time.0);
    let partition = ShardId(owned.map_or(0, |owned| owned.index().0));
    let simulation = &mut *kernel.simulation;
    let mut inbound = mem::take(&mut simulation.inbound);
    let mut ledgers = mem::take(&mut simulation.ledgers);
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
    for request in destroys.read() {
        if request.dimension != dimension {
            tracing::warn!(
                target: "voxel_update",
                dimension = ?request.dimension,
                pos = ?request.pos,
                "a block destroy names another dimension; dropping",
            );
            continue;
        }
        if !owned_columns::owns(owned, ColumnPos::from(request.pos)) {
            if let Some(outbox) = links.outbox.as_mut() {
                outbox.0.destroys.push(voxel_update::CellBlockDestroy {
                    pos: request.pos,
                    drops: request.drops,
                });
            }
            continue;
        }
        let stamp = SubTick::runtime(now, counter);
        counter = counter.saturating_add(1);
        inbound.push(Message {
            stamp,
            origin: ChainOrigin { stamp, partition },
            body: MessageBody::Destroy {
                pos: request.pos,
                drops: request.drops,
                limit: 512,
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

    let rules = &kernel.rules.0;
    let table = simulation.random_table.get_or_insert_default();
    let speed = random.speed.0;
    let seed = *simulation.random_seed.get_or_insert_with(|| {
        let world_seed = random.seed.as_deref().map_or(0, |seed| seed.0);
        let key = dimension_key.map_or("", |key| key.as_str());
        StreamSeed::new(world_seed as i64, key)
    });

    let facts = WorldFacts::new(definitions);
    let config = KernelConfig::default();
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
    let streams = outlets.streams.get(dimension).ok();
    let spread = PoolSpread(ComputeTaskPool::try_get().map(|pool| &**pool));
    let mut broadcast_cut = 0;
    for (phase, messages, chains) in phases {
        if phase == Phase::BlockEvents {
            match volume.view.random_ticks_mut().filter(|_| speed != 0) {
                Some(index) => index.draw(
                    |column| owned_columns::owns(owned, column),
                    speed,
                    seed.sections(now),
                    &mut simulation.random_listing,
                    &spread,
                ),
                None => simulation.random_listing = RandomListing::default(),
            }
            let picks = RandomPicks {
                columns: &simulation.random_listing.columns,
                hits: &simulation.random_listing.hits,
                speed,
                table,
                footprints: &simulation.footprints,
                seed,
            };
            mcrs_minecraft_simulation::random_step(
                &mut simulation.shard,
                now,
                &mut volume,
                &env,
                picks,
                &mut out,
                &spread,
            );
            mark_client_writes(&mut outlets.marks, &out.writes);
            outlets
                .marks
                .broadcast(&volume.view, streams, &mut outlets.changes);
            broadcast_cut = out.writes.len();
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
    if out.report.chain_overflows > 0 {
        tracing::warn!(
            target: "block_simulation",
            overflows = out.report.chain_overflows,
            first = ?out.report.first_chain_overflow,
            "neighbor update chain limit reached; further updates dropped",
        );
    }
    mark_client_writes(
        &mut outlets.marks,
        out.writes.get(broadcast_cut..).unwrap_or_default(),
    );

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
        && let Some(streams) = streams
    {
        publish_block_events(&out.delivered, streams, &mut outlets.changes);
    }
    for block in out.destroyed.iter().filter(|block| block.drops) {
        rules.drops(block.pos, block.state, &mut simulation.drops);
        for stack in simulation.drops.drain(..) {
            outlets.dropped.write(ItemDropped {
                at: block.pos,
                stack,
            });
        }
        if let Some(experience) = definitions.state(block.state).experience
            && let Some(provider) = definitions.experience_drop(experience)
        {
            let (min, max) = provider.bounds();
            if max > 0 {
                let amount = experience_amount(min, max, block.pos);
                if amount > 0 {
                    outlets.experience.write(ExperienceDropped {
                        at: block.pos,
                        amount,
                    });
                }
            }
        }
    }
    if let Some(outbox) = links.outbox.as_mut() {
        outbox.0.messages.append(&mut out.outbound);
        outbox.0.ledgers.append(&mut out.chains);
    }
    out.clear();
    simulation.out = out;
    inbound.clear();
    simulation.inbound = inbound;
    ledgers.clear();
    simulation.ledgers = ledgers;
}

fn experience_amount(min: i32, max: i32, at: BlockPos) -> i32 {
    let low = min.max(0);
    let high = max.max(low);
    if high == low {
        return low;
    }
    let mut hash = u64::from(at.x as u32)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(at.y as u32).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(u64::from(at.z as u32).wrapping_mul(0x1656_67B1_9E37_79F9));
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    hash ^= hash >> 33;
    let span = u64::try_from(i64::from(high) - i64::from(low) + 1).unwrap_or(1);
    low + i32::try_from(hash % span).unwrap_or(0)
}

/// Lists the positions that tick randomly in each loaded section once the block table landed.
/// From then on, block writes, the landing and unload of sections and the observers of
/// [`RandomTickIndexPlugin`] keep the index.
fn index_random_ticks<B: BlockBehaviour<WorldKinds> + Send + Sync + 'static>(world: &mut World) {
    let (Some(definitions), Some(rules)) = (
        world.get_resource::<Blocks>(),
        world.get_resource::<BlockRules<B>>(),
    ) else {
        return;
    };
    let rules = &rules.0;
    let table = RandomTickTable::new(definitions.states(), |state| rules.random_ticks(state));
    let footprints = Footprints::new(definitions.states(), |state| rules.reach(state));
    let mut index = RandomTickIndex::new(table.clone());
    let mut dimensions = world.query::<&SectionIndex>();
    let mut ticking = world.query::<Has<EntityTicking>>();
    for section_index in dimensions.iter(world) {
        for (pos, entity, blocks) in section_index.loaded() {
            let random_ticked = ticking.get(world, entity).unwrap_or(false);
            index.insert(pos, &PalettedSection(&blocks.0.0), random_ticked);
        }
    }
    world.insert_resource(index);
    let mut simulation = world.resource_mut::<BlockSimulation>();
    simulation.random_table = Some(table);
    simulation.footprints = footprints;
}

/// The threads of a task pool, or the calling thread without one.
#[derive(Clone, Copy, Debug)]
pub struct PoolSpread<'p>(pub Option<&'p TaskPool>);

impl Spread for PoolSpread<'_> {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        let Some(pool) = self
            .0
            .filter(|pool| pool.thread_num() > 1 && items.len() > 1)
        else {
            return Serial.spread(items, run);
        };
        // Tiles differ in cost, so each thread gets several small chunks.
        let chunk = items.len().div_ceil(pool.thread_num() * 4);
        let mut items = items;
        items.par_chunk_map_mut(pool, chunk, |_, chunk| {
            for item in chunk {
                run(item);
            }
        });
    }
}

fn mark_client_writes(marks: &mut BroadcastMarks, writes: &[BlockWrite]) {
    for write in writes {
        if write.flags.contains(BlockUpdateFlags::CLIENTS) {
            marks.mark(write.pos);
        }
    }
}

/// Publishes each event of `delivered` in a column that the muxer subscribes to. The muxer sends
/// it to the clients that hold the column and stand near the block.
fn publish_block_events(
    delivered: &[BlockEvent<Id<Block>>],
    streams: &ColumnStreams,
    changes: &mut MessageWriter<ColumnChange>,
) {
    for event in delivered {
        let column = ColumnPos::from(event.pos);
        if !streams.is_subscribed(column) {
            continue;
        }
        changes.write(ColumnChange {
            column,
            // The client takes only the low 8 bits of each argument.
            payload: PacketPayload::BlockEvent(ClientboundBlockEvent {
                pos: event.pos,
                action: event.a as u8,
                param: event.b as u8,
                block: VarInt(i32::from(event.block.number())),
            }),
            except: None,
        });
    }
}

fn cell_write(request: &BlockSetRequest) -> CellBlockWrite {
    CellBlockWrite {
        pos: request.pos,
        new_state: request.new_state,
        flags: request.flags,
        recursion_left: request.recursion_left,
    }
}
