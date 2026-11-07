use std::collections::{BTreeSet, HashMap, HashSet};

use crate::probe::{AIR, Host, ProbeBlock, ProbeFacts, ProbeFluid, ProbeKinds, STATES, section};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, BlockWrite, Cx, Env,
    Footprints, GameTick, KernelConfig, ListedSection, PartitionKind, Phase, PhaseInput,
    RandomPicks, RandomTickTable, RandomTicks, SemanticOrder, Serial, ShardId, SimulatedSections,
    SimulationVolume, Spread, StepAt, StepOutput, StreamSeed, TickPriority, step,
};
use mcrs_minecraft_volume::{BlockVolume, PaletteBuffer, SectionBlocks};
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};
use proptest::prelude::*;

/// Sections `0..LISTED` on x and z take picks: 3 × 3 block tiles of 4 × 4 sections. The world
/// holds `MARGIN` more sections on each side, so a serial re-run reads only loaded blocks.
const LISTED: i32 = 12;
const MARGIN: i32 = 3;
const FAR: i8 = 40;
const TILE_BLOCKS: i32 = 64;
const REGION_MARGIN: i32 = 32;
const FLAGS: BlockUpdateFlags = BlockUpdateFlags::CLIENTS.union(BlockUpdateFlags::KNOWN_SHAPE);

#[derive(Clone, Copy, Debug)]
enum Op {
    Draw(u8),
    Read(i8, i8),
    Write(i8, i8),
    Queue(i8, i8),
    Event(i8, i8),
    HasTick(i8, i8),
}

/// A tick or a block event that a pick queued. A copy of a queued item is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Item {
    BlockTick(BlockPos, ProbeBlock),
    FluidTick(BlockPos, ProbeFluid),
    Event(BlockEvent<ProbeBlock>),
}

impl Item {
    fn pos(self) -> BlockPos {
        match self {
            Self::BlockTick(pos, _) | Self::FluidTick(pos, _) => pos,
            Self::Event(event) => event.pos,
        }
    }
}

#[derive(Clone, Debug)]
struct Rule {
    ticks: bool,
    reach: u8,
    ops: Vec<Op>,
}

fn state(index: usize) -> BlockStateId {
    BlockStateId::checked(index as u16, STATES).expect("a probe state")
}

fn offset(at: BlockPos, dx: i8, dz: i8) -> BlockPos {
    BlockPos::new(at.x + i32::from(dx), at.y, at.z + i32::from(dz))
}

/// What a handler can do, in the kernel and in the reference.
trait Effects {
    fn read(&mut self, pos: BlockPos) -> Option<BlockStateId>;
    fn write(&mut self, pos: BlockPos, state: BlockStateId) -> bool;
    fn queue(&mut self, item: Item);
    fn has_tick(&mut self, pos: BlockPos, block: ProbeBlock) -> bool;
}

/// Each result feeds the next op, so a read or a write that differs changes what follows.
fn handle(
    rule: &Rule,
    effects: &mut impl Effects,
    at: BlockPos,
    at_state: BlockStateId,
    random: &mut LegacyRandom,
) {
    let mut acc = at_state.index() as u32;
    for op in &rule.ops {
        match *op {
            Op::Draw(bound) => {
                acc = acc.wrapping_mul(31) ^ random.next_u32_bound(u32::from(bound) + 1);
            }
            Op::Read(dx, dz) => {
                let read = effects
                    .read(offset(at, dx, dz))
                    .map_or(99, |s| s.index() as u32);
                acc = acc.wrapping_mul(31).wrapping_add(read);
            }
            Op::Write(dx, dz) => {
                // Air or any state past the void air.
                let written = match acc as usize % (STATES - 1) {
                    0 => AIR,
                    n => state(n + 1),
                };
                if effects.write(offset(at, dx, dz), written) {
                    acc = acc.wrapping_add(7);
                }
            }
            Op::Queue(dx, dz) => {
                let pos = offset(at, dx, dz);
                let kind = (acc / 2 % 4) as u16;
                effects.queue(match acc % 2 {
                    0 => Item::BlockTick(pos, ProbeBlock(kind)),
                    _ => Item::FluidTick(pos, ProbeFluid(kind)),
                });
            }
            Op::Event(dx, dz) => {
                let pos = offset(at, dx, dz);
                if let Some(there) = effects.read(pos) {
                    effects.queue(Item::Event(BlockEvent {
                        pos,
                        block: ProbeFacts.block_of(there),
                        a: acc as i32,
                        b: 0,
                    }));
                }
            }
            Op::HasTick(dx, dz) => {
                if effects.has_tick(offset(at, dx, dz), ProbeBlock((acc % 4) as u16)) {
                    acc = acc.wrapping_mul(5);
                }
            }
        }
    }
}

#[derive(Debug)]
struct Rules(Vec<Rule>);

impl Rules {
    fn get(&self, state: BlockStateId) -> Option<&Rule> {
        self.0.get(state.index())
    }

    fn table(&self) -> RandomTickTable {
        RandomTickTable::new((0..STATES).map(state), |s| self.random_ticks(s))
    }
}

impl BlockBehaviour<ProbeKinds> for Rules {
    fn random_ticks(&self, state: BlockStateId) -> RandomTicks {
        match self.get(state) {
            Some(rule) if rule.ticks => RandomTicks::BLOCK,
            _ => RandomTicks::empty(),
        }
    }

    fn reach(&self, state: BlockStateId) -> u8 {
        self.get(state)
            .map_or(Footprints::DEFAULT, |rule| rule.reach)
    }

    fn random_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        random: &mut LegacyRandom,
    ) {
        if let Some(rule) = self.get(state) {
            handle(rule, &mut Kernel(cx), at, state, random);
        }
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        _cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        true
    }
}

struct Kernel<'c, 'a, V, F>(&'c mut Cx<'a, ProbeKinds, V, F, Rules>);

impl<V: SimulationVolume, F: BlockFacts<ProbeKinds>> Effects for Kernel<'_, '_, V, F> {
    fn read(&mut self, pos: BlockPos) -> Option<BlockStateId> {
        self.0.state(pos).ok()
    }

    fn write(&mut self, pos: BlockPos, state: BlockStateId) -> bool {
        self.0.set(pos, state, FLAGS, 512)
    }

    fn queue(&mut self, item: Item) {
        match item {
            Item::BlockTick(pos, block) => {
                self.0
                    .schedule_block_tick(pos, block, 1, TickPriority::Normal);
            }
            Item::FluidTick(pos, fluid) => {
                self.0
                    .schedule_fluid_tick(pos, fluid, 1, TickPriority::Normal);
            }
            Item::Event(event) => {
                self.0
                    .queue_block_event(event.pos, event.block, event.a, event.b);
            }
        }
    }

    fn has_tick(&mut self, pos: BlockPos, block: ProbeBlock) -> bool {
        self.0.has_scheduled_block_tick(pos, block)
    }
}

/// The world after the steps: the writes in order, the scheduled ticks in queue order, the
/// block events that an event phase after the steps delivers, the number of re-runs and the
/// final state of each written position.
#[derive(Debug, Default, PartialEq, Eq)]
struct Outcome {
    writes: Vec<BlockWrite>,
    block_ticks: Vec<(BlockPos, ProbeBlock, i64)>,
    fluid_ticks: Vec<(BlockPos, ProbeFluid, i64)>,
    events: Vec<BlockEvent<ProbeBlock>>,
    reruns: u32,
    finals: Vec<(BlockPos, BlockStateId)>,
}

fn finals(host: &Host, writes: &[BlockWrite]) -> Vec<(BlockPos, BlockStateId)> {
    let positions: BTreeSet<(i32, i32, i32)> = writes.iter().map(|w| w.pos.into()).collect();
    positions
        .into_iter()
        .map(|pos| (pos.into(), host.state(pos.into())))
        .collect()
}

#[derive(Clone, Debug)]
struct Blob {
    sx: i32,
    sz: i32,
    state: usize,
    /// Out of 64 positions.
    density: u32,
    seed: u64,
}

fn world(blobs: &[Blob], table: &RandomTickTable) -> Host {
    let mut host = Host::new();
    for x in -MARGIN..LISTED + MARGIN {
        for z in -MARGIN..LISTED + MARGIN {
            host.insert(section(x, 0, z), AIR);
        }
    }
    for blob in blobs {
        let mut random = LegacyRandom::new(blob.seed);
        for index in 0..SectionPos::VOLUME {
            if random.next_u32_bound(64) < blob.density {
                host.put(
                    block_at(section(blob.sx, 0, blob.sz), index),
                    state(blob.state),
                );
            }
        }
    }
    host.set_random_ticks(table.clone());
    host
}

fn block_at(section: SectionPos, index: usize) -> BlockPos {
    let local = LocalPos::from_index(index);
    let origin = section.min_block();
    BlockPos::new(
        origin.x + i32::from(local.x()),
        origin.y + i32::from(local.y()),
        origin.z + i32::from(local.z()),
    )
}

fn seed() -> StreamSeed {
    StreamSeed::new(4, "minecraft:overworld")
}

fn tile_of(section: SectionPos) -> (i32, i32) {
    (section.x.div_euclid(4), section.z.div_euclid(4))
}

/// The listed sections grouped by block tile, the groups and the sections in each group shuffled.
fn listed(host: &Host, order: u64) -> Vec<ListedSection> {
    let mut random = LegacyRandom::new(order);
    let mut tiles: Vec<Vec<ListedSection>> = Vec::new();
    for tx in 0..LISTED / 4 {
        for tz in 0..LISTED / 4 {
            let mut group: Vec<ListedSection> = (0..4)
                .flat_map(|x| (0..4).map(move |z| section(tx * 4 + x, 0, tz * 4 + z)))
                .map(|section| ListedSection {
                    section,
                    ticking: u16::try_from(host.random_ticking(section).len())
                        .expect("4096 at most"),
                })
                .filter(|listed| listed.ticking > 0)
                .collect();
            shuffle(&mut group, &mut random);
            tiles.push(group);
        }
    }
    shuffle(&mut tiles, &mut random);
    tiles.into_iter().flatten().collect()
}

/// Runs the items on `threads` threads, each thread in a shuffled order of its own.
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

#[derive(Debug)]
struct Case {
    rules: Rules,
    blobs: Vec<Blob>,
    speed: u32,
    steps: i64,
    shuffle: u64,
}

fn kernel<S: Spread>(case: &Case, mut host: Host, spread: &S) -> Outcome {
    let rules = &case.rules;
    let table = rules.table();
    let footprints = Footprints::new((0..STATES).map(state), |s| rules.reach(s));
    let config = KernelConfig {
        event_drain_bound: u32::MAX,
        ..KernelConfig::default()
    };
    let env = Env {
        partition: ShardId(0),
        facts: &ProbeFacts,
        behaviour: rules,
        policy: &SemanticOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    for now in 0..case.steps {
        let listed = listed(&host, case.shuffle.wrapping_add(now as u64));
        let picks = RandomPicks {
            sections: &[],
            listed: &listed,
            speed: case.speed,
            table: &table,
            footprints: &footprints,
            seed: seed(),
        };
        mcrs_minecraft_simulation::random_step(
            &mut shard,
            GameTick(now),
            &mut host,
            &env,
            picks,
            &mut out,
            spread,
        );
    }
    let mut block_ticks: Vec<_> = {
        let queue = &*shard.block_ticks_mut();
        queue
            .sections()
            .filter_map(|section| queue.section_ticks(section))
            .flat_map(|ticks| ticks.active().collect::<Vec<_>>())
            .collect()
    };
    block_ticks.sort_by_key(|tick| tick.key);
    let mut fluid_ticks: Vec<_> = {
        let queue = &*shard.fluid_ticks_mut();
        queue
            .sections()
            .filter_map(|section| queue.section_ticks(section))
            .flat_map(|ticks| ticks.active().collect::<Vec<_>>())
            .collect()
    };
    fluid_ticks.sort_by_key(|tick| tick.key);
    let mut delivered = StepOutput::default();
    step(
        &mut shard,
        StepAt {
            now: GameTick(case.steps),
            phase: Phase::BlockEvents,
        },
        &mut host,
        &env,
        PhaseInput {
            inbound: &[],
            ledgers: &[],
        },
        &mut delivered,
    )
    .expect("no inbound messages");
    Outcome {
        finals: finals(&host, &out.writes),
        block_ticks: block_ticks
            .into_iter()
            .map(|tick| (tick.pos, tick.kind, tick.key.due.0))
            .collect(),
        fluid_ticks: fluid_ticks
            .into_iter()
            .map(|tick| (tick.pos, tick.kind, tick.key.due.0))
            .collect(),
        events: delivered.delivered,
        reruns: out.report.random_reruns,
        writes: out.writes,
    }
}

/// The reference: one thread, no overlay. It runs the picks in the order colour, tile, section,
/// pick. A pick that leaves the region of its tile is rolled back from a journal and runs again
/// after all tiles, in the order of the listed sections and the picks.
struct Reference<'a> {
    host: Host,
    rules: &'a Rules,
    table: &'a RandomTickTable,
    /// The ticking positions of each section, in ascending local index.
    lists: HashMap<SectionPos, Vec<u16>>,
    writes: Vec<BlockWrite>,
    /// The queued items in queue order, with the tick of their step. The steps never run a tick
    /// or deliver an event, so a copy of an item stays dropped.
    queued: Vec<(Item, i64)>,
    seen: HashSet<Item>,
    now: i64,
    /// The block tile whose region bounds the current pick.
    region: Option<(i32, i32)>,
    overflowed: bool,
    journal: Vec<(BlockPos, BlockStateId)>,
    pending: Vec<Item>,
}

impl Reference<'_> {
    fn inside(&self, pos: BlockPos, reach: u8) -> bool {
        let Some((tx, tz)) = self.region else {
            return true;
        };
        let reach = i32::from(reach);
        let span = |tile: i32| {
            tile * TILE_BLOCKS - REGION_MARGIN + reach
                ..=(tile + 1) * TILE_BLOCKS + REGION_MARGIN - 1 - reach
        };
        span(tx).contains(&pos.x) && span(tz).contains(&pos.z)
    }

    /// Fails the pick when `pos` is outside the region.
    fn reach_out(&mut self, pos: BlockPos) -> bool {
        let inside = self.inside(pos, 0);
        self.overflowed |= !inside;
        inside
    }

    fn put(&mut self, pos: BlockPos, new: BlockStateId) -> BlockStateId {
        let old = self.host.state(pos);
        self.host.put(pos, new);
        let list = self.lists.entry(SectionPos::from(pos)).or_default();
        let local = LocalPos::from(pos).index() as u16;
        match (self.table.ticks(new), list.binary_search(&local)) {
            (true, Err(slot)) => list.insert(slot, local),
            (false, Ok(slot)) => {
                list.remove(slot);
            }
            _ => {}
        }
        old
    }

    fn section_is_air(&self, pos: BlockPos) -> bool {
        let section = self
            .host
            .section(SectionPos::from(pos))
            .expect("a loaded section");
        let mut buffer = PaletteBuffer::default();
        section
            .palette(&mut buffer)
            .iter()
            .all(|(state, _)| ProbeFacts.is_air(state))
    }

    fn pick(&mut self, at: BlockPos, random: &mut LegacyRandom) {
        let at_state = self.host.state(at);
        if !self.table.ticks(at_state) {
            return;
        }
        if !self.inside(at, self.rules.reach(at_state)) {
            self.overflowed = true;
            return;
        }
        let rules = self.rules;
        if let Some(rule) = rules.get(at_state) {
            handle(rule, self, at, at_state, random);
        }
    }

    /// Runs one pick, in the region of the tile of `region` if any. Returns `false` and leaves
    /// no trace when the pick left the region.
    fn bounded_pick(
        &mut self,
        at: BlockPos,
        random: &mut LegacyRandom,
        region: Option<(i32, i32)>,
    ) -> bool {
        self.region = region;
        let written = self.writes.len();
        self.pick(at, random);
        let journal = std::mem::take(&mut self.journal);
        let pending = std::mem::take(&mut self.pending);
        if std::mem::take(&mut self.overflowed) {
            for (pos, old) in journal.into_iter().rev() {
                self.put(pos, old);
            }
            self.writes.truncate(written);
            return false;
        }
        for item in pending {
            if self.seen.insert(item) {
                self.queued.push((item, self.now + 1));
            }
        }
        true
    }

    fn run(mut self, case: &Case) -> Outcome {
        let mut reruns = 0u32;
        for now in 0..case.steps {
            self.now = now;
            let listed = listed(&self.host, case.shuffle.wrapping_add(now as u64));
            let streams = seed().sections(GameTick(now));
            let mut order: Vec<usize> = (0..listed.len()).collect();
            order.sort_by_key(|&i| {
                let (x, z) = tile_of(listed[i].section);
                ((x & 1, z & 1), (x, z))
            });
            let mut deferred = Vec::new();
            for index in order {
                let section = listed[index].section;
                let mut random = streams.stream(section);
                for pick in 0..case.speed {
                    let drawn = random.next_u32_bound(4096) as usize;
                    let Some(&local) = self.lists.get(&section).and_then(|list| list.get(drawn))
                    else {
                        continue;
                    };
                    let at = block_at(section, usize::from(local));
                    let replay = random.clone();
                    if !self.bounded_pick(at, &mut random, Some(tile_of(section))) {
                        deferred.push(((index, pick), at, replay));
                    }
                }
            }
            deferred.sort_by_key(|(order, _, _)| *order);
            reruns += deferred.len() as u32;
            for (_, at, mut random) in deferred {
                self.bounded_pick(at, &mut random, None);
            }
        }
        let mut outcome = Outcome {
            finals: finals(&self.host, &self.writes),
            reruns,
            writes: self.writes,
            ..Outcome::default()
        };
        for (item, due) in self.queued {
            match item {
                Item::BlockTick(pos, block) => outcome.block_ticks.push((pos, block, due)),
                Item::FluidTick(pos, fluid) => outcome.fluid_ticks.push((pos, fluid, due)),
                Item::Event(event) => {
                    if ProbeFacts.block_of(self.host.state(event.pos)) == event.block {
                        outcome.events.push(event);
                    }
                }
            }
        }
        outcome
    }
}

impl Effects for Reference<'_> {
    fn read(&mut self, pos: BlockPos) -> Option<BlockStateId> {
        self.reach_out(pos).then(|| self.host.state(pos))
    }

    fn write(&mut self, pos: BlockPos, new: BlockStateId) -> bool {
        if !self.reach_out(pos)
            || (ProbeFacts.is_air(new) && self.section_is_air(pos))
            || self.host.state(pos) == new
        {
            return false;
        }
        let old = self.put(pos, new);
        self.journal.push((pos, old));
        self.writes.push(BlockWrite {
            pos,
            old,
            new,
            flags: FLAGS,
        });
        // The placement side effect of the new state has its own bound.
        if !self.inside(pos, self.rules.reach(new)) {
            self.overflowed = true;
        }
        true
    }

    fn queue(&mut self, item: Item) {
        if self.reach_out(item.pos()) {
            self.pending.push(item);
        }
    }

    fn has_tick(&mut self, pos: BlockPos, block: ProbeBlock) -> bool {
        let item = Item::BlockTick(pos, block);
        self.reach_out(pos) && (self.seen.contains(&item) || self.pending.contains(&item))
    }
}

fn reference(case: &Case, host: Host) -> Outcome {
    let table = case.rules.table();
    let mut lists: HashMap<SectionPos, Vec<u16>> = HashMap::new();
    // Outside the blobs the world is air.
    for blob in &case.blobs {
        let at = section(blob.sx, 0, blob.sz);
        if lists.contains_key(&at) {
            continue;
        }
        let list = (0..SectionPos::VOLUME)
            .filter(|&index| table.ticks(host.state(block_at(at, index))))
            .map(|index| index as u16)
            .collect();
        lists.insert(at, list);
    }
    Reference {
        host,
        rules: &case.rules,
        table: &table,
        lists,
        writes: Vec::new(),
        queued: Vec::new(),
        seen: HashSet::new(),
        now: 0,
        region: None,
        overflowed: false,
        journal: Vec::new(),
        pending: Vec::new(),
    }
    .run(case)
}

fn reach() -> impl Strategy<Value = u8> {
    prop_oneof![
        3 => 0u8..=3,
        1 => prop::sample::select(vec![8u8, 30, 31, 32, 33, 48, Footprints::UNBOUNDED]),
    ]
}

fn delta() -> impl Strategy<Value = i8> {
    prop_oneof![4 => -3i8..=3, 1 => -FAR..=FAR]
}

fn op() -> impl Strategy<Value = Op> {
    let at = || (delta(), delta());
    prop_oneof![
        (0u8..16).prop_map(Op::Draw),
        at().prop_map(|(dx, dz)| Op::Read(dx, dz)),
        at().prop_map(|(dx, dz)| Op::Write(dx, dz)),
        at().prop_map(|(dx, dz)| Op::Queue(dx, dz)),
        at().prop_map(|(dx, dz)| Op::Event(dx, dz)),
        at().prop_map(|(dx, dz)| Op::HasTick(dx, dz)),
    ]
}

fn rule() -> impl Strategy<Value = Rule> {
    (
        prop::bool::weighted(0.75),
        reach(),
        prop::collection::vec(op(), 0..8),
    )
        .prop_map(|(ticks, reach, ops)| Rule { ticks, reach, ops })
}

fn blob() -> impl Strategy<Value = Blob> {
    (
        -MARGIN..LISTED + MARGIN,
        -MARGIN..LISTED + MARGIN,
        2..STATES,
        1u32..=64,
        any::<u64>(),
    )
        .prop_map(|(sx, sz, state, density, seed)| Blob {
            sx,
            sz,
            state,
            density,
            seed,
        })
}

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec(rule(), STATES),
        prop::collection::vec(blob(), 4..40),
        1u32..96,
        1i64..4,
        any::<u64>(),
    )
        .prop_map(|(mut rules, blobs, speed, steps, shuffle)| {
            for air in rules.iter_mut().take(2) {
                air.ticks = false;
            }
            Case {
                rules: Rules(rules),
                blobs,
                speed,
                steps,
                shuffle,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// SIM-MODEL-102(a), SIM-RND-001: on any world, footprints and handlers, on one thread in
    /// order or on 1 to 16 threads that run the tasks of a colour in any order, the step leaves
    /// the writes, the queued ticks and events and the re-run count of the serial order.
    #[test]
    fn colour_phases_on_any_threads_equal_the_serial_order(
        case in case(),
        threads in 0usize..=16,
        order in any::<u64>(),
    ) {
        let world = world(&case.blobs, &case.rules.table());
        let expected = reference(&case, world.clone());
        let found = match threads {
            0 => kernel(&case, world, &Serial),
            _ => kernel(&case, world, &Shuffled { threads, seed: order }),
        };
        prop_assert_eq!(found, expected);
    }
}
