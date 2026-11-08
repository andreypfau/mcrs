use crate::probe::{
    A, AIR, B, C, Host, NEEDS_SUPPORT, ProbeFacts, ProbeKinds, STACK, STATES, at, section,
};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockShard, BlockUpdateFlags, BlockWrite, Cx, Env, Footprints,
    GameTick, KernelConfig, PartitionKind, RandomHit, RandomPicks, RandomTickTable, RandomTicks,
    SemanticOrder, Serial, ShardId, SimulationVolume, Spread, StepOutput, StreamSeed, block_tile,
    random_step,
};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

/// `A` spreads east into air and turns into `B` against a block, but waits for a far or
/// unbounded block; the east neighbour can be in the
/// next block tile. `NEEDS_SUPPORT` claims the default bound but reads 100 blocks east after its
/// write of air; `STACK` has no bound. Both turn into air, then `C`.
struct Spreading;

const FAR: BlockStateId = NEEDS_SUPPORT;
const UNBOUNDED: BlockStateId = STACK;

impl BlockBehaviour<ProbeKinds> for Spreading {
    fn random_ticks(&self, state: BlockStateId) -> RandomTicks {
        if [A, FAR, UNBOUNDED].contains(&state) {
            RandomTicks::BLOCK
        } else {
            RandomTicks::empty()
        }
    }

    fn reach(&self, state: BlockStateId) -> u8 {
        if state == UNBOUNDED {
            Footprints::UNBOUNDED
        } else {
            Footprints::DEFAULT
        }
    }

    fn random_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _random: &mut LegacyRandom,
    ) {
        if state != A {
            cx.set(at, AIR, BlockUpdateFlags::CLIENTS, 512);
            let _ = cx.state(BlockPos::new(at.x + 100, at.y, at.z));
            cx.set(at, C, BlockUpdateFlags::CLIENTS, 512);
            return;
        }
        let east = BlockPos::new(at.x + 1, at.y, at.z);
        match cx.state(east) {
            Ok(AIR) => {
                cx.set(east, A, BlockUpdateFlags::CLIENTS, 512);
            }
            Ok(C) | Ok(A) | Ok(B) => {
                cx.set(at, B, BlockUpdateFlags::CLIENTS, 512);
            }
            _ => {}
        }
    }
}

/// Splits the items over four threads.
struct Threads;

impl Spread for Threads {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        let chunk = items.len().div_ceil(4).max(1);
        std::thread::scope(|scope| {
            for part in items.chunks_mut(chunk) {
                scope.spawn(move || {
                    for item in part {
                        run(item);
                    }
                });
            }
        });
    }
}

const SPEED: u32 = 1024;
const TICKS: i64 = 40;

/// Sections 0..12 on x and z: 3 × 3 block tiles of all four colours. Air with a line of `A` at
/// the east edge of each section, except in the sections of `C` that hold a row of far or unbounded blocks east of a row of `A`.
fn host(table: &RandomTickTable) -> Host {
    let mut host = Host::new();
    for x in 0..12 {
        for z in 0..12 {
            let target = section(x, 0, z);
            match marker_of(x, z) {
                Some(marker) => {
                    host.insert(target, C);
                    for row in 0..16 {
                        host.put(at(x * 16 + 13, 0, z * 16 + row), A);
                        host.put(marker_at(x, z, row), marker);
                    }
                }
                None => {
                    host.insert(target, AIR);
                    for y in 0..16 {
                        host.put(at(x * 16 + 15, y, z * 16 + (y % 16)), A);
                    }
                }
            }
        }
    }
    host.set_random_ticks(table.clone());
    host
}

fn marker_of(x: i32, z: i32) -> Option<BlockStateId> {
    match (x * 12 + z) % 17 {
        0 | 1 if x % 2 == 0 => Some(FAR),
        0 | 1 => Some(UNBOUNDED),
        _ => None,
    }
}

fn marker_at(x: i32, z: i32, row: i32) -> BlockPos {
    at(x * 16 + 14, 0, z * 16 + row)
}

/// The listed sections, grouped by block tile in the order of the colour phases.
fn listed(host: &Host) -> Vec<SectionPos> {
    let tile = |section: &SectionPos| block_tile(ColumnPos::from(*section));
    let mut listed: Vec<SectionPos> = (0..12)
        .flat_map(|x| (0..12).map(move |z| section(x, 0, z)))
        .filter(|&section| !host.random_ticking(section).is_empty())
        .collect();
    listed.sort_by_key(|section| ([tile(section).x & 1, tile(section).z & 1], tile(section)));
    listed
}

fn states(host: &Host) -> Vec<BlockStateId> {
    (0..192)
        .flat_map(|x| (0..16).flat_map(move |y| (0..192).map(move |z| at(x, y, z))))
        .map(|pos| host.block(pos).expect("a loaded position"))
        .collect()
}

struct Run {
    states: Vec<BlockStateId>,
    writes: Vec<BlockWrite>,
    reruns: u32,
    /// The far and unbounded blocks, unless an undone pick let an `A` spread into them.
    markers_kept: bool,
}

/// Runs the steps with the hits of each step given to `random_step` in `batches`.
fn run<S: Spread>(spread: &S, batches: impl Fn(Vec<RandomHit>) -> Vec<Vec<RandomHit>>) -> Run {
    let rules = Spreading;
    let states_of = || (0..STATES).filter_map(|n| BlockStateId::checked(n as u16, STATES));
    let table = RandomTickTable::new(states_of(), |state| rules.random_ticks(state));
    let footprints = Footprints::new(states_of(), |state| rules.reach(state));
    let mut host = host(&table);
    let config = KernelConfig::default();
    let env = Env {
        partition: ShardId(0),
        facts: &ProbeFacts,
        behaviour: &rules,
        policy: &SemanticOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    for now in 0..TICKS {
        for batch in batches(host.draw_hits(&listed(&host), SPEED, now)) {
            let picks = RandomPicks {
                columns: &[],
                hits: &batch,
                speed: SPEED,
                table: &table,
                footprints: &footprints,
                seed: StreamSeed::new(4, "minecraft:overworld"),
            };
            random_step(
                &mut shard,
                GameTick(now),
                &mut host,
                &env,
                picks,
                &mut out,
                spread,
            );
        }
    }
    let markers_kept = (0..12)
        .flat_map(|x| (0..12).map(move |z| (x, z)))
        .filter_map(|(x, z)| Some((marker_of(x, z)?, x, z)))
        .flat_map(|(marker, x, z)| (0..16).map(move |row| (marker, marker_at(x, z, row))))
        .all(|(marker, pos)| [marker, C].contains(&host.state(pos)));
    Run {
        markers_kept,
        states: states(&host),
        writes: out.writes,
        reruns: out.report.random_reruns,
    }
}

/// SIM-MODEL-102(a), SIM-RND-001: the colour phases give the same result on one thread and on
/// four, and the result of running the hits one at a time in the order of the phases. A pick
/// that leaves its region, by its bound or by what it reads, is undone and runs again.
#[test]
fn colour_phases_equal_one_thread_and_the_serial_order_of_the_hits() {
    let whole = |hits: Vec<RandomHit>| vec![hits];
    let one = run(&Serial, whole);
    let four = run(&Threads, whole);
    let serial = run(&Serial, |hits: Vec<RandomHit>| {
        hits.into_iter().map(|hit| vec![hit]).collect()
    });

    assert!(one.reruns > 0, "some far and unbounded picks ran again");
    assert!(one.markers_kept, "an undone pick leaves no trace");
    assert!(
        one.writes.len() > 1000,
        "the picks spread across the tiles, {} writes",
        one.writes.len()
    );
    assert_eq!(one.writes, four.writes, "four threads write as one");
    assert!(
        one.states == four.states,
        "four threads leave the blocks of one"
    );
    assert!(
        one.states == serial.states,
        "the phases leave the blocks of the serial order"
    );
}
