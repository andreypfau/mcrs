use crate::SimulationVolume;
use crate::access::{PartitionKind, SimulatedSections};
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::Cx;
use crate::event_queue::BlockEvent;
use crate::event_queue::Drained;
use crate::footprints::Footprints;
use crate::message::{Message, MessageBody};
use crate::output::{StepError, StepOutput};
use crate::policy::OrderingPolicy;
use crate::scalar::GameTick;
use crate::shard::{BlockShard, Env, Phase, PhaseInput, StepAt};
use crate::tick_queue::{Collected, Kinds};
use crate::tile_task::{Mode, Spread, Tiled, run_tiled};
use mcrs_minecraft_volume::{Access, Volume};
use mcrs_voxel::BlockPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;
use std::mem::take;

fn runs_ticks<V: SimulatedSections>(volume: &V, section: SectionPos) -> bool {
    volume.access(section) == Access::Write
        && volume
            .properties(section)
            .contains(crate::SectionProperties::RUNS_SCHEDULED_TICKS)
}

fn delivers_events<V: SimulatedSections>(volume: &V, section: SectionPos) -> bool {
    volume.access(section) == Access::Write
        && volume
            .properties(section)
            .contains(crate::SectionProperties::DELIVERS_BLOCK_EVENTS)
}

fn validate<K: Kinds, V: Volume>(
    volume: &V,
    kind: PartitionKind,
    inbound: &[Message<K>],
) -> Result<(), StepError> {
    for message in inbound {
        let section = message.section();
        if matches!(volume.access(section), Access::Read | Access::Foreign) {
            return Err(StepError::NotOwned { section });
        }
        let queued = !matches!(
            message.body,
            MessageBody::Set { .. } | MessageBody::Update(_)
        );
        if kind == PartitionKind::Cell && queued {
            return Err(StepError::NoQueues { section });
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the step reads or writes"
)]
pub fn step<K, V, F, B, P>(
    shard: &mut BlockShard<K>,
    at: StepAt,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    input: PhaseInput<'_, K>,
    out: &mut StepOutput<K>,
) -> Result<(), StepError>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
{
    shard.chain.reset();
    validate(&*volume, env.kind, input.inbound)?;
    let mut cx = Cx::new(shard, volume, out, env, Some(memory), shared, at.now);
    ingress(&mut cx, input);
    match at.phase {
        Phase::Ingress => {}
        Phase::BlockTicks => block_phase(&mut cx, env.policy),
        Phase::FluidTicks => fluid_phase(&mut cx, env.policy),
        Phase::BlockEvents => event_phase(&mut cx),
    }
    Ok(())
}

fn is_update<K: Kinds>(message: &Message<K>) -> bool {
    matches!(message.body, MessageBody::Update(_))
}

fn ingress<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, input: PhaseInput<'_, K>)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let mut carried = FxHashMap::default();
    let mut rest = input.inbound;
    while let Some((first, tail)) = rest.split_first() {
        if !is_update(first) {
            cx.apply(*first);
            rest = tail;
            continue;
        }
        let length = rest
            .iter()
            .take_while(|message| is_update(message) && message.origin == first.origin)
            .count();
        let Some((run, after)) = rest.split_at_checked(length) else {
            return;
        };
        let offered = carried.get(&first.origin).copied().unwrap_or_else(|| {
            input
                .ledgers
                .iter()
                .filter(|ledger| ledger.origin == first.origin)
                .map(|ledger| ledger.offered)
                .max()
                .unwrap_or(0)
        });
        let ended = cx.seed_chain(first.origin, offered, run);
        carried.insert(first.origin, ended);
        rest = after;
    }
}

fn block_phase<K, V, F, B, P>(cx: &mut Cx<'_, K, V, F, B>, policy: &P)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
{
    let mut collected = take(&mut cx.shard.collected_blocks);
    let volume = &*cx.volume;
    policy.collect_due(
        &mut cx.shard.block_ticks,
        cx.now,
        cx.config.tick_budget,
        |section| runs_ticks(volume, section),
        &mut cx.shard.scratch,
        &mut collected,
    );
    cx.shard.ticking.clear();
    cx.shard
        .ticking
        .extend(collected.iter().map(|tick| (tick.pos, tick.kind)));
    for tick in &collected {
        cx.run_block_tick(*tick);
    }
    cx.shard.ticking.clear();
    collected.clear();
    cx.shard.collected_blocks = collected;
}

fn fluid_phase<K, V, F, B, P>(cx: &mut Cx<'_, K, V, F, B>, policy: &P)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
{
    let mut collected = take(&mut cx.shard.collected_fluids);
    let volume = &*cx.volume;
    policy.collect_due(
        &mut cx.shard.fluid_ticks,
        cx.now,
        cx.config.tick_budget,
        |section| runs_ticks(volume, section),
        &mut cx.shard.scratch,
        &mut collected,
    );
    for tick in &collected {
        cx.run_fluid_tick(*tick);
    }
    collected.clear();
    cx.shard.collected_fluids = collected;
}

fn event_phase<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    loop {
        let volume = &*cx.volume;
        let next = cx
            .shard
            .events
            .next_eligible(cx.config.event_drain_bound, |section| {
                delivers_events(volume, section)
            });
        match next {
            Drained::Event(queued) => cx.deliver(queued.event),
            Drained::Done(end) => {
                let report = &mut cx.out.report;
                report.events_deferred = report.events_deferred.saturating_add(end.deferred);
                report.events_left_by_bound = report
                    .events_left_by_bound
                    .saturating_add(end.left_by_bound);
                return;
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the step reads or writes"
)]
pub fn step_tiled<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    at: StepAt,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    footprints: &Footprints,
    input: PhaseInput<'_, K>,
    out: &mut StepOutput<K>,
    spread: &S,
) -> Result<(), StepError>
where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
{
    shard.chain.reset();
    validate(&*volume, env.kind, input.inbound)?;
    let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, at.now);
    ingress(&mut cx, input);
    match at.phase {
        Phase::Ingress => {}
        Phase::BlockTicks => {
            let collected = collect_blocks(&mut cx, env.policy);
            run_block_ticks(
                shard, volume, env, memory, shared, footprints, out, at.now, spread, &collected,
            );
            shard.ticking.clear();
            shard.collected_blocks = collected;
            shard.collected_blocks.clear();
        }
        Phase::FluidTicks => {
            let collected = collect_fluids(&mut cx, env.policy);
            run_fluid_ticks(
                shard, volume, env, memory, shared, footprints, out, at.now, spread, &collected,
            );
            shard.collected_fluids = collected;
            shard.collected_fluids.clear();
        }
        Phase::BlockEvents => {
            run_events(
                shard, volume, env, memory, shared, footprints, out, at.now, spread,
            );
        }
    }
    Ok(())
}

fn collect_blocks<K, V, F, B, P>(
    cx: &mut Cx<'_, K, V, F, B>,
    policy: &P,
) -> Vec<Collected<K::Block>>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
{
    let mut collected = take(&mut cx.shard.collected_blocks);
    let volume = &*cx.volume;
    policy.collect_due(
        &mut cx.shard.block_ticks,
        cx.now,
        cx.config.tick_budget,
        |section| runs_ticks(volume, section),
        &mut cx.shard.scratch,
        &mut collected,
    );
    cx.shard.ticking.clear();
    cx.shard
        .ticking
        .extend(collected.iter().map(|tick| (tick.pos, tick.kind)));
    collected
}

fn collect_fluids<K, V, F, B, P>(
    cx: &mut Cx<'_, K, V, F, B>,
    policy: &P,
) -> Vec<Collected<K::Fluid>>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
{
    let mut collected = take(&mut cx.shard.collected_fluids);
    let volume = &*cx.volume;
    policy.collect_due(
        &mut cx.shard.fluid_ticks,
        cx.now,
        cx.config.tick_budget,
        |section| runs_ticks(volume, section),
        &mut cx.shard.scratch,
        &mut collected,
    );
    collected
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
fn run_block_ticks<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    footprints: &Footprints,
    out: &mut StepOutput<K>,
    now: GameTick,
    spread: &S,
    collected: &[Collected<K::Block>],
) where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
{
    let positions: Vec<BlockPos> = collected.iter().map(|tick| tick.pos).collect();
    let tiled = run_tiled(
        shard,
        volume,
        env,
        memory,
        shared,
        footprints,
        out,
        now,
        spread,
        &positions,
        Mode::Sequence,
        &|cx, index| {
            if let Some(tick) = collected.get(index) {
                cx.run_block_tick(*tick);
            }
        },
        &|_, _| {},
    );
    let again: Vec<usize> = match tiled {
        Tiled::Done { reruns, left } => {
            out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
            reruns
        }
        Tiled::Redo { left } => {
            out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
            (0..collected.len()).collect()
        }
    };
    let mut cx = Cx::new(shard, volume, out, env, Some(memory), shared, now);
    for tick in again.iter().filter_map(|&index| collected.get(index)) {
        cx.run_block_tick(*tick);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
fn run_fluid_ticks<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    footprints: &Footprints,
    out: &mut StepOutput<K>,
    now: GameTick,
    spread: &S,
    collected: &[Collected<K::Fluid>],
) where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
{
    let positions: Vec<BlockPos> = collected.iter().map(|tick| tick.pos).collect();
    let tiled = run_tiled(
        shard,
        volume,
        env,
        memory,
        shared,
        footprints,
        out,
        now,
        spread,
        &positions,
        Mode::Sequence,
        &|cx, index| {
            if let Some(tick) = collected.get(index) {
                cx.run_fluid_tick(*tick);
            }
        },
        &|_, _| {},
    );
    let again: Vec<usize> = match tiled {
        Tiled::Done { reruns, left } => {
            out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
            reruns
        }
        Tiled::Redo { left } => {
            out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
            (0..collected.len()).collect()
        }
    };
    let mut cx = Cx::new(shard, volume, out, env, Some(memory), shared, now);
    for tick in again.iter().filter_map(|&index| collected.get(index)) {
        cx.run_fluid_tick(*tick);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
fn run_events<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    footprints: &Footprints,
    out: &mut StepOutput<K>,
    now: GameTick,
    spread: &S,
) where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
{
    let mut budget = env.config.event_drain_bound;
    loop {
        if budget == 0 {
            let left = u32::try_from(shard.events.pending_len()).unwrap_or(u32::MAX);
            out.report.events_left_by_bound = out.report.events_left_by_bound.saturating_add(left);
            return;
        }
        let mut batch: Vec<BlockEvent<K::Block>> = Vec::new();
        loop {
            let next = shard
                .events
                .next_eligible(budget, |section| delivers_events(&*volume, section));
            match next {
                Drained::Event(queued) => batch.push(queued.event),
                Drained::Done(end) => {
                    let report = &mut out.report;
                    report.events_deferred = report.events_deferred.saturating_add(end.deferred);
                    report.events_left_by_bound = report
                        .events_left_by_bound
                        .saturating_add(end.left_by_bound);
                    break;
                }
            }
        }
        if batch.is_empty() {
            return;
        }
        budget = budget.saturating_sub(u32::try_from(batch.len()).unwrap_or(u32::MAX));
        for event in &batch {
            shard.events.hold(*event);
        }
        let positions: Vec<BlockPos> = batch.iter().map(|event| event.pos).collect();
        let tiled = run_tiled(
            shard,
            volume,
            env,
            memory,
            shared,
            footprints,
            out,
            now,
            spread,
            &positions,
            Mode::Sequence,
            &|cx, index| {
                if let Some(event) = batch.get(index) {
                    cx.deliver(*event);
                }
            },
            &|shard, index| {
                if let Some(event) = batch.get(index) {
                    shard.events.release(*event);
                }
            },
        );
        let again: Vec<usize> = match tiled {
            Tiled::Done { reruns, left } => {
                out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
                reruns
            }
            Tiled::Redo { left } => {
                out.report.tile_reruns = out.report.tile_reruns.saturating_add(left);
                (0..batch.len()).collect()
            }
        };
        let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, now);
        for event in again.iter().filter_map(|&index| batch.get(index)) {
            cx.shard.events.release(*event);
            cx.deliver(*event);
        }
        if shard.events.is_empty() {
            return;
        }
    }
}
