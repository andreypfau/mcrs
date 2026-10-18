use crate::SimulationVolume;
use crate::access::{PartitionKind, SimulatedSections};
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::Cx;
use crate::event_queue::Drained;
use crate::message::{Message, MessageBody};
use crate::output::{StepError, StepOutput};
use crate::policy::OrderingPolicy;
use crate::shard::{BlockShard, Env, Phase, PhaseInput, StepAt};
use crate::tick_queue::Kinds;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_volume::{Access, Volume};
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

pub fn step<K, V, F, B, P>(
    shard: &mut BlockShard<K>,
    at: StepAt,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
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
    let mut cx = Cx::new(shard, volume, out, env, at.now);
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
        // A chain's continuations need not be contiguous: a later run of the same chain
        // continues from where the earlier one ended, not from the ledger again.
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
