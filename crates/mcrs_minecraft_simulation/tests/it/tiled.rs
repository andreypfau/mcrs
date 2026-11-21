use std::collections::BTreeMap;

use crate::probe::{
    A, AIR, B, C, Host, ProbeBlock, ProbeFacts, ProbeKinds, at, block_tick_message, event_message,
    section,
};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, BlockWrite, ChainOrigin,
    Cx, Env, Footprints, GameTick, KernelConfig, Message, MessageBody, PartitionKind, Phase,
    PhaseInput, Serial, ShardId, SimulationVolume, Spread, StepAt, StepOutput, SubTick, TickKey,
    TickPriority, VanillaOrder, step, step_tiled,
};
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

const SECTIONS: i32 = 12;
const STEPS: i64 = 24;
const FLAGS: BlockUpdateFlags = BlockUpdateFlags::CLIENTS.union(BlockUpdateFlags::KNOWN_SHAPE);
const FAR: i32 = 40;

struct Rules;

fn next_state(state: BlockStateId) -> BlockStateId {
    match state {
        A => B,
        B => C,
        _ => A,
    }
}

fn hop(at: BlockPos, salt: i32) -> BlockPos {
    let mix = at.x.wrapping_mul(31) ^ at.z.wrapping_mul(17) ^ salt;
    let reach = if mix.rem_euclid(7) == 0 { FAR } else { 3 };
    BlockPos::new(
        at.x + (mix.rem_euclid(reach * 2 + 1) - reach),
        at.y,
        at.z + ((mix / 3).rem_euclid(reach * 2 + 1) - reach),
    )
}

impl BlockBehaviour<ProbeKinds> for Rules {
    type Memory = ();

    type Shared = ();

    fn reach(&self, state: BlockStateId) -> u8 {
        if state == C { FAR as u8 } else { 3 }
    }

    fn block_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
    ) {
        let target = hop(at, 5);
        let seen = cx.state(target).ok().map_or(0, |seen| seen.index());
        let written = if seen.is_multiple_of(2) {
            next_state(state)
        } else {
            state
        };
        cx.set(target, written, FLAGS, 512);
        let again = hop(at, 11);
        cx.schedule_block_tick(again, ProbeFacts.block_of(written), 1, TickPriority::Normal);
        if seen.is_multiple_of(3) {
            cx.queue_block_event(at, ProbeFacts.block_of(state), seen as i32, 0);
        }
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        a: i32,
        _b: i32,
    ) -> bool {
        let target = hop(at, a);
        cx.set(target, next_state(state), FLAGS, 512);
        true
    }
}

struct Shuffled {
    threads: usize,
    seed: u64,
}

impl Spread for Shuffled {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        let mut items: Vec<&mut T> = items.iter_mut().collect();
        shuffle(&mut items, &mut LegacyRandom::new(self.seed));
        let mut parts: Vec<Vec<&mut T>> = (0..self.threads).map(|_| Vec::new()).collect();
        for (n, item) in items.into_iter().enumerate() {
            parts[n % self.threads].push(item);
        }
        std::thread::scope(|scope| {
            for part in parts.into_iter().filter(|part| !part.is_empty()) {
                scope.spawn(move || {
                    for item in part {
                        run(item);
                    }
                });
            }
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    blocks: BTreeMap<(i32, i32, i32), u16>,
    ticks: Vec<(BlockPos, ProbeBlock, TickKey)>,
    events: Vec<BlockEvent<ProbeBlock>>,
    writes: Vec<BlockWrite>,
}

fn world() -> Host {
    let mut host = Host::new();
    for x in 0..SECTIONS {
        for z in 0..SECTIONS {
            host.insert(section(x, 0, z), AIR);
        }
    }
    let mut random = LegacyRandom::new(7);
    for _ in 0..600 {
        let x = random.next_u32_bound((SECTIONS * 16) as u32) as i32;
        let z = random.next_u32_bound((SECTIONS * 16) as u32) as i32;
        let state = match random.next_u32_bound(3) {
            0 => A,
            1 => B,
            _ => C,
        };
        host.put(at(x, 0, z), state);
    }
    host
}

fn seeds(host: &Host) -> Vec<Message<ProbeKinds>> {
    let mut messages = Vec::new();
    let mut counter = 0;
    for x in 0..SECTIONS * 16 {
        for z in 0..SECTIONS * 16 {
            let pos = at(x, 0, z);
            let state = host.state(pos);
            if state == AIR {
                continue;
            }
            let stamp = SubTick::runtime(GameTick(0), counter);
            counter += 1;
            messages.push(Message {
                stamp,
                origin: ChainOrigin {
                    stamp,
                    partition: ShardId(0),
                },
                body: MessageBody::BlockTick {
                    pos,
                    block: ProbeFacts.block_of(state),
                    due: GameTick(1),
                    priority: TickPriority::Normal,
                },
            });
        }
    }
    messages
}

fn run<S: Spread>(spread: &S) -> Outcome {
    let mut host = world();
    let footprints = Footprints::new([A, B, C], |state| Rules.reach(state));
    let config = KernelConfig::default();
    let env = Env {
        partition: ShardId(0),
        facts: &ProbeFacts,
        behaviour: &Rules,
        policy: &VanillaOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    let mut events = Vec::new();
    let mut writes = Vec::new();
    let seeded = seeds(&host);
    for now in 0..STEPS {
        let inbound: &[Message<ProbeKinds>] = if now == 0 { &seeded } else { &[] };
        for phase in [Phase::BlockTicks, Phase::FluidTicks, Phase::BlockEvents] {
            let input = PhaseInput {
                inbound: if phase == Phase::BlockTicks {
                    inbound
                } else {
                    &[]
                },
                ledgers: &[],
            };
            step_tiled(
                &mut shard,
                StepAt {
                    now: GameTick(now),
                    phase,
                },
                &mut host,
                &env,
                &mut (),
                &(),
                &footprints,
                input,
                &mut out,
                spread,
            )
            .expect("owned sections");
        }
        events.append(&mut out.delivered);
        writes.append(&mut out.writes);
        out.clear();
    }
    let mut blocks = BTreeMap::new();
    for x in 0..SECTIONS * 16 {
        for z in 0..SECTIONS * 16 {
            let state = host.state(at(x, 0, z));
            if state != AIR {
                blocks.insert((x, 0, z), state.number());
            }
        }
    }
    let queue = shard.block_ticks();
    let mut ticks: Vec<_> = queue
        .sections()
        .filter_map(|section| queue.section_ticks(section))
        .flat_map(|ticks| ticks.active().collect::<Vec<_>>())
        .map(|tick| (tick.pos, tick.kind, tick.key))
        .collect();
    ticks.sort_by_key(|(pos, kind, key)| (*key, pos.x, pos.z, kind.0));
    Outcome {
        blocks,
        ticks,
        events,
        writes,
    }
}

#[test]
fn ticks_and_events_on_four_threads_give_the_result_of_one_thread() {
    let one = run(&Serial);
    assert!(!one.writes.is_empty(), "the ticks write blocks");
    assert!(
        !one.events.is_empty(),
        "the ticks queue events that are delivered"
    );
    for seed in [1, 2, 3] {
        let four = run(&Shuffled { threads: 4, seed });
        assert_eq!(four.blocks, one.blocks, "the blocks of seed {seed}");
        assert_eq!(four.ticks, one.ticks, "the queue of seed {seed}");
        assert_eq!(four.events, one.events, "the events of seed {seed}");
        assert_eq!(four.writes, one.writes, "the writes of seed {seed}");
    }
}

#[test]
fn the_same_world_crosses_tile_regions_so_the_serial_path_is_exercised() {
    let mut host = world();
    let footprints = Footprints::new([A, B, C], |state| Rules.reach(state));
    let config = KernelConfig::default();
    let env = Env {
        partition: ShardId(0),
        facts: &ProbeFacts,
        behaviour: &Rules,
        policy: &VanillaOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    let seeded = seeds(&host);
    let mut far = 0_usize;
    for x in 0..SECTIONS * 16 {
        for z in 0..SECTIONS * 16 {
            let pos = at(x, 0, z);
            if host.state(pos) == C && hop(pos, 5) != pos {
                far += 1;
            }
        }
    }
    assert!(
        far > 0,
        "some ticks reach {FAR} blocks and leave their region"
    );
    step_tiled(
        &mut shard,
        StepAt {
            now: GameTick(1),
            phase: Phase::BlockTicks,
        },
        &mut host,
        &env,
        &mut (),
        &(),
        &footprints,
        PhaseInput {
            inbound: &seeded,
            ledgers: &[],
        },
        &mut out,
        &Serial,
    )
    .expect("owned sections");
    assert!(!out.writes.is_empty());
    assert!(
        out.report.tile_reruns > 0,
        "ticks that leave their region run again on the serial path"
    );
    assert_eq!(
        SectionPos::from(at(17, 0, 17)),
        section(1, 0, 1),
        "the sections of the world are the ones the tiles cover"
    );
}

enum Path<'a> {
    Scalar,
    Tiled(&'a Shuffled),
    Single,
}

struct Lab {
    host: Host,
    shard: BlockShard<ProbeKinds>,
    footprints: Footprints,
    config: KernelConfig,
}

impl Lab {
    fn new(config: KernelConfig) -> Self {
        Self {
            host: world(),
            shard: BlockShard::new(),
            footprints: Footprints::new([A, B, C], |_| Footprints::DEFAULT),
            config,
        }
    }

    fn tick<R: BlockBehaviour<ProbeKinds, Memory = (), Shared = ()>>(
        &mut self,
        rules: &R,
        now: i64,
        inbound: &[Message<ProbeKinds>],
        path: &Path<'_>,
    ) -> StepOutput<ProbeKinds> {
        let env = Env {
            partition: ShardId(0),
            facts: &ProbeFacts,
            behaviour: rules,
            policy: &VanillaOrder,
            kind: PartitionKind::Shard,
            config: &self.config,
        };
        let mut out = StepOutput::default();
        for phase in [Phase::BlockTicks, Phase::FluidTicks, Phase::BlockEvents] {
            let input = PhaseInput {
                inbound: if phase == Phase::BlockTicks {
                    inbound
                } else {
                    &[]
                },
                ledgers: &[],
            };
            let at = StepAt {
                now: GameTick(now),
                phase,
            };
            let result = match path {
                Path::Scalar => step(
                    &mut self.shard,
                    at,
                    &mut self.host,
                    &env,
                    &mut (),
                    &(),
                    input,
                    &mut out,
                ),
                Path::Tiled(spread) => step_tiled(
                    &mut self.shard,
                    at,
                    &mut self.host,
                    &env,
                    &mut (),
                    &(),
                    &self.footprints,
                    input,
                    &mut out,
                    *spread,
                ),
                Path::Single => step_tiled(
                    &mut self.shard,
                    at,
                    &mut self.host,
                    &env,
                    &mut (),
                    &(),
                    &self.footprints,
                    input,
                    &mut out,
                    &Serial,
                ),
            };
            result.expect("owned sections");
        }
        out
    }
}

const P: BlockPos = BlockPos::new(70, 0, 70);
const Q: BlockPos = BlockPos::new(72, 0, 70);
const MARK_P: BlockPos = BlockPos::new(70, 0, 72);
const MARK_Q: BlockPos = BlockPos::new(72, 0, 72);

struct Membership;

impl BlockBehaviour<ProbeKinds> for Membership {
    type Memory = ();

    type Shared = ();

    fn block_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        _state: BlockStateId,
    ) {
        let (other, mark) = if at == P { (Q, MARK_P) } else { (P, MARK_Q) };
        let pending = cx.will_tick_this_tick(other, ProbeFacts.block_of(A));
        cx.set(mark, if pending { B } else { C }, FLAGS, 512);
    }
}

fn membership_marks(path: &Path<'_>) -> (BlockStateId, BlockStateId, bool) {
    let mut lab = Lab::new(KernelConfig::default());
    lab.host.put(P, A);
    lab.host.put(Q, A);
    let block = ProbeFacts.block_of(A);
    let seeds = [
        block_tick_message(0, P, block, 1),
        block_tick_message(1, Q, block, 1),
    ];
    lab.tick(&Membership, 1, &seeds, path);
    let queue = lab.shard.block_ticks_mut();
    let left = !queue.contains(P, block) && !queue.contains(Q, block);
    (lab.host.state(MARK_P), lab.host.state(MARK_Q), left)
}

#[test]
fn a_tiled_tick_sees_a_later_tick_as_pending_and_an_earlier_one_as_consumed() {
    for path in [Path::Scalar, Path::Single] {
        let (mark_p, mark_q, left) = membership_marks(&path);
        assert_eq!(mark_p, B, "P runs first and sees Q still pending");
        assert_eq!(mark_q, C, "Q runs second and sees P consumed");
        assert!(left, "both ticks left the queue");
    }
}

const EVENT_A: BlockPos = BlockPos::new(70, 0, 80);
const EVENT_B: BlockPos = BlockPos::new(72, 0, 80);

struct Requeue;

impl BlockBehaviour<ProbeKinds> for Requeue {
    type Memory = ();

    type Shared = ();

    fn block_event<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        a: i32,
        _b: i32,
    ) -> bool {
        if at == EVENT_A && a == 0 {
            cx.queue_block_event(EVENT_B, ProbeFacts.block_of(state), 0, 0);
        }
        if at == EVENT_B && a == 0 {
            cx.queue_block_event(EVENT_A, ProbeFacts.block_of(state), 1, 0);
        }
        true
    }
}

fn requeue_deliveries(path: &Path<'_>) -> (Vec<(BlockPos, i32)>, Vec<(BlockPos, i32)>) {
    let mut lab = Lab::new(KernelConfig::default());
    lab.host.put(EVENT_A, A);
    lab.host.put(EVENT_B, A);
    let block = ProbeFacts.block_of(A);
    let seeds = [
        event_message(0, EVENT_A, block),
        event_message(1, EVENT_B, block),
    ];
    let first = lab.tick(&Requeue, 1, &seeds, path);
    let second = lab.tick(&Requeue, 2, &[], path);
    let positions = |out: StepOutput<ProbeKinds>| -> Vec<(BlockPos, i32)> {
        out.delivered
            .iter()
            .map(|event| (event.pos, event.a))
            .collect()
    };
    (positions(first), positions(second))
}

#[test]
fn a_copy_of_a_later_pending_event_is_dropped_and_a_copy_of_a_delivered_one_is_kept() {
    for path in [Path::Scalar, Path::Single] {
        let (first, second) = requeue_deliveries(&path);
        assert_eq!(
            first,
            vec![(EVENT_A, 0), (EVENT_B, 0), (EVENT_A, 1)],
            "the copy of the pending B is dropped, the new event at A runs in the same drain"
        );
        assert!(second.is_empty(), "nothing is left for the next drain");
    }
}

#[test]
fn the_event_drain_bound_holds_for_the_whole_tiled_phase() {
    let config = KernelConfig {
        event_drain_bound: 1,
        ..KernelConfig::default()
    };
    for path in [Path::Scalar, Path::Single] {
        let mut lab = Lab::new(config);
        lab.host.put(EVENT_A, A);
        lab.host.put(EVENT_B, A);
        let block = ProbeFacts.block_of(A);
        let seeds = [
            event_message(0, EVENT_A, block),
            event_message(1, EVENT_B, block),
        ];
        let delivered = |out: StepOutput<ProbeKinds>| -> Vec<(BlockPos, i32)> {
            out.delivered
                .iter()
                .map(|event| (event.pos, event.a))
                .collect()
        };
        let first = delivered(lab.tick(&Requeue, 1, &seeds, &path));
        assert_eq!(first, vec![(EVENT_A, 0)], "one event per phase");
        let second = delivered(lab.tick(&Requeue, 2, &[], &path));
        assert_eq!(
            second,
            vec![(EVENT_B, 0)],
            "the copy of B was dropped while B waited, so B runs once"
        );
        let third = delivered(lab.tick(&Requeue, 3, &[], &path));
        assert_eq!(
            third,
            vec![(EVENT_A, 1)],
            "the event B queued waits a phase"
        );
    }
}

const WEST_EDGE: BlockPos = BlockPos::new(63, 0, 40);
const EAST_EDGE: BlockPos = BlockPos::new(64, 0, 40);

struct Replacer;

impl BlockBehaviour<ProbeKinds> for Replacer {
    type Memory = ();

    type Shared = ();

    fn block_event<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        _state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        if at == EAST_EDGE {
            cx.set(WEST_EDGE, B, FLAGS, 512);
        } else {
            cx.set(at, C, FLAGS, 512);
        }
        true
    }
}

fn replacer_outcome(path: &Path<'_>) -> (Vec<BlockPos>, BlockStateId, u32) {
    let mut lab = Lab::new(KernelConfig::default());
    lab.host.put(WEST_EDGE, A);
    lab.host.put(EAST_EDGE, A);
    let block = ProbeFacts.block_of(A);
    let seeds = [
        event_message(0, EAST_EDGE, block),
        event_message(1, WEST_EDGE, block),
    ];
    let out = lab.tick(&Replacer, 1, &seeds, path);
    let delivered = out.delivered.iter().map(|event| event.pos).collect();
    (delivered, lab.host.state(WEST_EDGE), out.report.tile_redos)
}

#[test]
fn an_earlier_event_in_a_later_tile_still_runs_before_the_event_it_invalidates() {
    let (delivered, west, redos) = replacer_outcome(&Path::Scalar);
    assert_eq!(delivered, vec![EAST_EDGE]);
    assert_eq!(west, B, "the first event replaced the block of the second");
    assert_eq!(redos, 0);
    for seed in [1, 2] {
        let shuffled = Shuffled { threads: 4, seed };
        let (delivered, west, redos) = replacer_outcome(&Path::Tiled(&shuffled));
        assert_eq!(
            delivered,
            vec![EAST_EDGE],
            "the tiles give the scalar result"
        );
        assert_eq!(west, B);
        assert_eq!(redos, 1, "the phase ran again on the serial path");
    }
}

#[test]
fn the_tiled_step_gives_the_result_of_the_scalar_step() {
    let scalar = run_path(&Path::Scalar);
    let single = run_path(&Path::Single);
    assert_eq!(single.blocks, scalar.blocks);
    assert_eq!(single.ticks, scalar.ticks);
    assert_eq!(single.events, scalar.events);
    assert_eq!(single.writes, scalar.writes);
}

fn run_path(path: &Path<'_>) -> Outcome {
    let mut lab = Lab::new(KernelConfig::default());
    lab.footprints = Footprints::new([A, B, C], |state| Rules.reach(state));
    let seeded = seeds(&lab.host);
    let mut events = Vec::new();
    let mut writes = Vec::new();
    for now in 0..STEPS {
        let inbound: &[Message<ProbeKinds>] = if now == 0 { &seeded } else { &[] };
        let mut out = lab.tick(&Rules, now, inbound, path);
        events.append(&mut out.delivered);
        writes.append(&mut out.writes);
    }
    let mut blocks = BTreeMap::new();
    for x in 0..SECTIONS * 16 {
        for z in 0..SECTIONS * 16 {
            let state = lab.host.state(at(x, 0, z));
            if state != AIR {
                blocks.insert((x, 0, z), state.number());
            }
        }
    }
    let queue = lab.shard.block_ticks();
    let mut ticks: Vec<_> = queue
        .sections()
        .filter_map(|section| queue.section_ticks(section))
        .flat_map(|ticks| ticks.active().collect::<Vec<_>>())
        .map(|tick| (tick.pos, tick.kind, tick.key))
        .collect();
    ticks.sort_by_key(|(pos, kind, key)| (*key, pos.x, pos.z, kind.0));
    Outcome {
        blocks,
        ticks,
        events,
        writes,
    }
}
