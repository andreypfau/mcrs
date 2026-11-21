use std::collections::BTreeMap;

use crate::probe::{A, AIR, B, C, Host, ProbeBlock, ProbeFacts, ProbeKinds, at, section};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, BlockWrite, ChainOrigin,
    Cx, Env, Footprints, GameTick, KernelConfig, Message, MessageBody, PartitionKind, Phase,
    PhaseInput, Serial, ShardId, SimulationVolume, Spread, StepAt, StepOutput, SubTick,
    TickPriority, VanillaOrder, step_tiled,
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
    ticks: Vec<(BlockPos, ProbeBlock, i64)>,
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
        .map(|tick| (tick.pos, tick.kind, tick.key.due.0))
        .collect();
    ticks.sort_by_key(|(pos, kind, due)| (*due, pos.x, pos.z, kind.0));
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
