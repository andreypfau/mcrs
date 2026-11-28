use std::mem;

use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Has, Query, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::common_conditions::{not, on_message, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemCondition, SystemSet};
use bevy_ecs::system::{Local, Res, ResMut, SystemParam};
use bevy_ecs::world::World;
use bevy_tasks::{ComputeTaskPool, ParallelSliceMut, TaskPool};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_engine_world::voxel_update::{self, CellBlockWrite, VoxelUpdateSet};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{BlockTicking, EntityTicking};
use mcrs_minecraft_engine_world::world::storage::random_ticks::{
    RandomListing, RandomTickIndex, RandomTickIndexPlugin,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_minecraft_item_stack::ItemStack;
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
use mcrs_minecraft_volume::{BlockVolume, PalettedSection};
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};
use rustc_hash::FxHashSet;

use crate::block_update::{BroadcastMarks, remove_replaced_block_entities};
use crate::bus::PacketPayload;
use crate::cell_mail::CellOutbox;
use crate::cell_volume::CellVolume;
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;
use crate::column_ticks::LandedTicks;
use crate::coordinator::clock::GameTime;
use crate::random_tick_speed::RandomTickSpeed;
use crate::world_kinds::{WorldFacts, WorldKinds};
use mcrs_minecraft_engine_world::block_drops::{
    BlockBroken, BlockDrops, DroppingBlock, ItemDropped,
};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_region_disk::column_ticks::{ColumnTicks, StoredTick};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InertBlocks;

impl BlockBehaviour<WorldKinds> for InertBlocks {}

impl BlockDrops for InertBlocks {}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct BlockRules<B>(pub B);

#[derive(Resource, Clone, Copy, Debug)]
pub struct DimensionBlockRules(pub fn(&mut SubApp));

impl Default for DimensionBlockRules {
    fn default() -> Self {
        Self(|sub_app| {
            sub_app.add_plugins(BlockSimulationPlugin::new(InertBlocks));
        })
    }
}

pub struct BlockSimulationPlugin<B> {
    rules: B,
}

impl<B> BlockSimulationPlugin<B> {
    #[must_use]
    pub fn new(rules: B) -> Self {
        Self { rules }
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockSimulationSet;

impl<B: BlockBehaviour<WorldKinds> + BlockDrops + Clone + Send + 'static> Plugin
    for BlockSimulationPlugin<B>
{
    fn build(&self, app: &mut App) {
        app.insert_resource(BlockRules(self.rules.clone()));
        app.init_resource::<BlockSimulation>();
        app.init_resource::<RandomTickSpeed>();
        app.init_resource::<LandedTicks>();
        app.init_resource::<BroadcastMarks>();
        app.add_message::<ItemDropped>();
        app.add_message::<BlockBroken>();
        app.add_plugins(RandomTickIndexPlugin);
        app.add_systems(
            FixedUpdate,
            (
                index_random_ticks::<B>.run_if(
                    resource_exists::<Blocks>.and_then(not(resource_exists::<RandomTickIndex>)),
                ),
                drop_broken_blocks::<B>.run_if(on_message::<BlockBroken>),
                run_block_simulation::<B>,
            )
                .chain()
                .in_set(BlockSimulationSet),
        );
        app.configure_sets(FixedUpdate, BlockSimulationSet.in_set(TickSet::WorldWrite));
        app.add_systems(
            FixedPostUpdate,
            remove_replaced_block_entities
                .in_set(VoxelUpdateSet::NetworkSync)
                .run_if(on_message::<BlockPlaced>),
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenTicks {
    pub ticks: ColumnTicks,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedTicks {
    pub ticks: ColumnTicks,
    pub changed: bool,
}

#[derive(Resource, Default)]
pub struct BlockSimulation {
    shard: BlockShard<WorldKinds>,
    out: StepOutput<WorldKinds>,
    inbound: Vec<Message<WorldKinds>>,
    ledgers: Vec<ChainLedger>,
    held: FxHashSet<SectionPos>,
    activated: FxHashSet<ColumnPos>,
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

    fn hold(&mut self, ticks: &ColumnTicks) {
        hold_list(self.shard.block_ticks_mut(), &mut self.held, &ticks.block);
        hold_list(self.shard.fluid_ticks_mut(), &mut self.held, &ticks.fluid);
    }

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

    pub fn copy_column(
        &self,
        column: ColumnPos,
        rows: impl IntoIterator<Item = i32>,
        now: GameTick,
    ) -> CopiedTicks {
        let mut block = Taken::default();
        let mut fluid = Taken::default();
        for y in rows {
            let section = SectionPos::new(column.x, y, column.z);
            if let Some(ticks) = self.shard.block_ticks().section_ticks(section) {
                block.add(ticks, now);
            }
            if let Some(ticks) = self.shard.fluid_ticks().section_ticks(section) {
                fluid.add(ticks, now);
            }
        }
        let changed = self.activated.contains(&column) || block.has_active() || fluid.has_active();
        CopiedTicks {
            ticks: ColumnTicks {
                block: block.into_list(),
                fluid: fluid.into_list(),
            },
            changed,
        }
    }

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

#[derive(SystemParam)]
struct RandomTicking<'w> {
    speed: Res<'w, RandomTickSpeed>,
    seed: Option<Res<'w, WorldSeed>>,
    pace: Option<Res<'w, TickPace>>,
}

#[derive(SystemParam)]
struct StepOutlets<'w, 's> {
    marks: ResMut<'w, BroadcastMarks>,
    streams: Query<'w, 's, &'static ColumnStreams>,
    changes: MessageWriter<'w, ColumnChange>,
    dropped: DropOutlet<'w>,
}

#[derive(SystemParam)]
struct DropOutlet<'w> {
    dropped: MessageWriter<'w, ItemDropped>,
    game_rules: Option<Res<'w, GameRules>>,
}

impl DropOutlet<'_> {
    fn write(&mut self, at: BlockPos, stacks: impl Iterator<Item = ItemStack>) {
        let on = self
            .game_rules
            .as_deref()
            .is_none_or(|rules| rules.is_on(GameRule::BlockDrops));
        if on {
            self.dropped
                .write_batch(stacks.map(|stack| ItemDropped { at, stack }));
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the sync point, the four phases and the hand-off of their output are one ordered step"
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "advances the scheduled and random block ticks of every loaded column each tick"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the broadcast marks allocate only for a section whose blocks change in this tick, and the update entries move into the outbound packet"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
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
    let owned = links.owned.as_deref();
    let Ok((dimension, dimension_key)) = dims.single() else {
        requests.clear();
        return;
    };
    let Some(definitions) = kernel.definitions.as_deref() else {
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
    let runs_normally = random.pace.as_deref().is_none_or(TickPace::runs_normally);
    let speed = if runs_normally { random.speed.0 } else { 0 };
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
            if !runs_normally {
                continue;
            }
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
    let world_seed = random.seed.as_deref().map_or(0, |seed| seed.0);
    for block in out.destroyed.iter().filter(|block| block.drops) {
        let dropping = DroppingBlock {
            at: block.pos,
            state: block.state,
            tool: None,
            breaker: None,
            tick: kernel.time.0.cast_unsigned(),
            world_seed,
        };
        let view = &volume.view;
        rules.drops(&dropping, |pos| view.block(pos).ok(), &mut simulation.drops);
        outlets.dropped.write(block.pos, simulation.drops.drain(..));
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

fn drop_broken_blocks<B: BlockDrops + Send + Sync + 'static>(
    mut harvests: MessageReader<BlockBroken>,
    world: WorldView,
    dims: Query<Entity, With<Dimension>>,
    rules: Res<BlockRules<B>>,
    time: Res<GameTime>,
    seed: Option<Res<WorldSeed>>,
    mut drops: Local<Vec<ItemStack>>,
    mut broken_now: Local<Vec<BlockPos>>,
    mut dropped: DropOutlet,
) {
    broken_now.clear();
    let Some(view) = dims
        .single()
        .ok()
        .and_then(|dimension| world.get(dimension))
    else {
        harvests.clear();
        return;
    };
    let tick = time.0.cast_unsigned();
    let world_seed = seed.as_deref().map_or(0, |seed| seed.0);
    for harvest in harvests.read() {
        let parts = [Some(harvest.at), harvest.other.map(|(at, _)| at)];
        if parts.iter().flatten().any(|at| broken_now.contains(at)) {
            continue;
        }
        broken_now.extend(parts.into_iter().flatten());
        if let Some((at, state)) = harvest.other {
            let removed = DroppingBlock {
                at,
                state,
                tool: None,
                breaker: None,
                tick,
                world_seed,
            };
            let after_hit = |pos| {
                if pos == harvest.at {
                    Some(harvest.left)
                } else {
                    view.block(pos).ok()
                }
            };
            rules.0.drops(&removed, after_hit, &mut drops);
            dropped.write(at, drops.drain(..));
        }
        if !harvest.harvests {
            continue;
        }
        let broken = DroppingBlock {
            at: harvest.at,
            state: harvest.state,
            tool: None,
            breaker: Some(harvest.breaker),
            tick,
            world_seed,
        };
        rules
            .0
            .drops(&broken, |pos| view.block(pos).ok(), &mut drops);
        dropped.write(harvest.at, drops.drain(..));
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "runs once, because its run condition stops it as soon as the index exists"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::exclusive_system,
        reason = "the observers of `EntityTicking` and of section despawns read the index, so it must exist before any later command applies"
    )
)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_block::block_keys::Block;
    use mcrs_minecraft_simulation::{ScheduledTick, SubTick, TickKey, TickPriority};
    use mcrs_voxel::BlockPos;

    #[test]
    fn a_periodic_copy_reports_a_tick_scheduled_since_the_column_landed() {
        let mut simulation = BlockSimulation::default();
        let column = ColumnPos::new(2, 3);
        let pos = BlockPos::new(35, 70, 50);
        let key = TickKey {
            due: GameTick(10),
            priority: TickPriority::Normal,
            sequence: SubTick::runtime(GameTick(1), 1),
        };
        simulation.shard.block_ticks_mut().schedule(ScheduledTick {
            pos,
            kind: Block::Stone.id(),
            key,
        });

        let copied = simulation.copy_column(column, 4..5, GameTick(1));
        assert!(
            copied.changed,
            "a scheduled tick is not reported as a change"
        );
        assert_eq!(copied.ticks.block.len(), 1);
        assert!(
            !simulation.copy_column(column, 0..1, GameTick(1)).changed,
            "a tick in another section marks the column changed"
        );
    }
}
