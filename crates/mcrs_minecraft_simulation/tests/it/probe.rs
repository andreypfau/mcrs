use mcrs_minecraft_core::{BlockPos, BlockStateId, BoundingBox, Direction, SectionPos};
use mcrs_minecraft_simulation::SimulationVolume;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, ChainLedger, ChainOrigin,
    Cx, Env, GameTick, KernelConfig, Kinds, Message, MessageBody, PartitionKind, Phase, PhaseInput,
    SectionProperties, SemanticOrder, ShardId, SimulatedSections, StepAt, StepError, StepOutput,
    SubTick, TickPriority, step,
};
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, Denied, PalettedSection, PalettedSectionMut, SparseVolume,
    Unavailable, Volume,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProbeBlock(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProbeFluid(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeKinds;

impl Kinds for ProbeKinds {
    type Block = ProbeBlock;
    type Fluid = ProbeFluid;
}

pub const STATES: usize = 16;

const fn state(number: u16) -> BlockStateId {
    match BlockStateId::checked(number, STATES) {
        Some(state) => state,
        None => panic!("state outside the probe table"),
    }
}

pub const AIR: BlockStateId = state(0);
pub const VOID_AIR: BlockStateId = state(1);
pub const A: BlockStateId = state(2);
pub const A_OTHER: BlockStateId = state(3);
pub const B: BlockStateId = state(4);
pub const C: BlockStateId = state(5);
pub const FLOWING: BlockStateId = state(7);
pub const SOURCE: BlockStateId = state(8);
pub const RAIL_FLAT: BlockStateId = state(9);
pub const RAIL_SLOPED: BlockStateId = state(10);
pub const STACK: BlockStateId = state(11);
pub const STACK_CHANGED: BlockStateId = state(12);
pub const NEEDS_SUPPORT: BlockStateId = state(13);
pub const NEEDS_SUPPORT_WET: BlockStateId = state(14);
pub const WIRE: BlockStateId = state(15);

pub const BLOCK_A: ProbeBlock = ProbeBlock(10);
pub const BLOCK_B: ProbeBlock = ProbeBlock(11);
pub const BLOCK_C: ProbeBlock = ProbeBlock(12);
pub const BLOCK_NEEDS_SUPPORT: ProbeBlock = ProbeBlock(42);
pub const WATER: ProbeFluid = ProbeFluid(1);

const BLOCKS: [u16; STATES] = [0, 1, 10, 10, 11, 12, 13, 20, 21, 30, 30, 40, 41, 42, 42, 44];

pub struct ProbeFacts;

impl BlockFacts<ProbeKinds> for ProbeFacts {
    fn block_of(&self, state: BlockStateId) -> ProbeBlock {
        ProbeBlock(BLOCKS.get(state.index()).copied().unwrap_or(0))
    }

    fn fluid_of(&self, state: BlockStateId) -> Option<ProbeFluid> {
        (state == FLOWING || state == SOURCE || state == NEEDS_SUPPORT_WET).then_some(WATER)
    }

    fn void_air(&self) -> BlockStateId {
        VOID_AIR
    }

    fn is_air(&self, state: BlockStateId) -> bool {
        state == AIR || state == VOID_AIR
    }

    fn is_rail(&self, state: BlockStateId) -> bool {
        self.block_of(state) == ProbeBlock(30)
    }

    fn is_redstone_wire(&self, state: BlockStateId) -> bool {
        state == WIRE
    }

    fn legacy_block_of_fluid(&self, state: BlockStateId) -> BlockStateId {
        self.fluid_of(state).map_or(AIR, |_| SOURCE)
    }
}

pub fn at(x: i32, y: i32, z: i32) -> BlockPos {
    BlockPos::new(x, y, z)
}

pub fn section(x: i32, y: i32, z: i32) -> SectionPos {
    SectionPos::new(x, y, z)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    BlockTick,
    FluidTick,
    BlockEvent,
    Placed,
    Neighbour,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    WillTick,
    HasScheduled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Hook {
        now: i64,
        hook: Hook,
        pos: BlockPos,
        state: BlockStateId,
    },
    Removed {
        now: i64,
        pos: BlockPos,
        old: BlockStateId,
        moved_by_piston: bool,
    },
    Placed {
        now: i64,
        pos: BlockPos,
        new: BlockStateId,
        old: BlockStateId,
        moved_by_piston: bool,
    },
    Answer {
        now: i64,
        query: Query,
        pos: BlockPos,
        value: bool,
    },
    Set {
        now: i64,
        pos: BlockPos,
        result: bool,
    },
    Read {
        now: i64,
        pos: BlockPos,
        result: Result<BlockStateId, Unavailable>,
    },
    Neighbour {
        pos: BlockPos,
        state: BlockStateId,
        source: ProbeBlock,
        moved_by_piston: bool,
    },
    Shape {
        pos: BlockPos,
        state: BlockStateId,
        from: Direction,
        neighbour: BlockPos,
        neighbour_state: BlockStateId,
    },
    Mark(u8),
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    ScheduleBlock {
        pos: BlockPos,
        block: ProbeBlock,
        delay: i32,
    },
    ScheduleFluid {
        pos: BlockPos,
        fluid: ProbeFluid,
        delay: i32,
    },
    QueueEvent {
        pos: BlockPos,
        block: ProbeBlock,
        a: i32,
        b: i32,
    },
    Set {
        pos: BlockPos,
        state: BlockStateId,
        flags: u32,
        limit: u16,
    },
    AskWillTick {
        pos: BlockPos,
        block: ProbeBlock,
    },
    AskHasScheduled {
        pos: BlockPos,
        block: ProbeBlock,
    },
    Read {
        pos: BlockPos,
    },
    UpdateNeighbour {
        target: BlockPos,
        source: ProbeBlock,
    },
    UpdateNeighbours {
        source: BlockPos,
        block: ProbeBlock,
    },
    UpdateNeighbourWithState {
        target: BlockPos,
        state: BlockStateId,
        source: ProbeBlock,
    },
    Mark(u8),
}

#[derive(Clone, Debug)]
pub struct Rule {
    hook: Hook,
    at: BlockPos,
    actions: Vec<Action>,
    once: bool,
    answer: bool,
}

impl Rule {
    pub fn on(hook: Hook, at: BlockPos, actions: Vec<Action>) -> Self {
        Self {
            hook,
            at,
            actions,
            once: false,
            answer: true,
        }
    }

    pub fn once(mut self) -> Self {
        self.once = true;
        self
    }

    pub fn answering(mut self, answer: bool) -> Self {
        self.answer = answer;
        self
    }
}

pub struct ProbeBehaviour {
    rules: Vec<Rule>,
    fired: Mutex<Vec<bool>>,
    log: Mutex<Vec<Entry>>,
    nesting: AtomicU32,
    deepest: AtomicU32,
}

impl ProbeBehaviour {
    pub fn new(rules: Vec<Rule>) -> Self {
        let fired = Mutex::new(vec![false; rules.len()]);
        Self {
            rules,
            fired,
            log: Mutex::new(Vec::new()),
            nesting: AtomicU32::new(0),
            deepest: AtomicU32::new(0),
        }
    }

    pub fn deepest_nesting(&self) -> u32 {
        self.deepest.load(Ordering::Relaxed)
    }

    pub fn log(&self) -> Vec<Entry> {
        self.log.lock().expect("log lock").clone()
    }

    fn push(&self, entry: Entry) {
        self.log.lock().expect("log lock").push(entry);
    }

    fn matching(&self, hook: Hook, at: BlockPos) -> Vec<(Vec<Action>, bool)> {
        let mut fired = self.fired.lock().expect("fired lock");
        let mut matched = Vec::new();
        for (index, rule) in self.rules.iter().enumerate() {
            let spent = rule.once && fired.get(index).copied().unwrap_or(false);
            if rule.hook == hook && rule.at == at && !spent {
                if let Some(slot) = fired.get_mut(index) {
                    *slot = true;
                }
                matched.push((rule.actions.clone(), rule.answer));
            }
        }
        matched
    }

    fn run<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        hook: Hook,
        at: BlockPos,
    ) -> bool {
        let mut answer = true;
        for (actions, rule_answer) in self.matching(hook, at) {
            answer = rule_answer;
            for action in actions {
                self.act(cx, action);
            }
        }
        answer
    }

    fn act<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        action: Action,
    ) {
        let now = cx.now().0;
        match action {
            Action::ScheduleBlock { pos, block, delay } => {
                cx.schedule_block_tick(pos, block, delay, TickPriority::Normal);
            }
            Action::ScheduleFluid { pos, fluid, delay } => {
                cx.schedule_fluid_tick(pos, fluid, delay, TickPriority::Normal);
            }
            Action::QueueEvent { pos, block, a, b } => cx.queue_block_event(pos, block, a, b),
            Action::Set {
                pos,
                state,
                flags,
                limit,
            } => {
                let flags = BlockUpdateFlags::from_bits_retain(flags);
                let result = cx.set(pos, state, flags, limit);
                self.push(Entry::Set { now, pos, result });
            }
            Action::AskWillTick { pos, block } => {
                let value = cx.will_tick_this_tick(pos, block);
                self.push(Entry::Answer {
                    now,
                    query: Query::WillTick,
                    pos,
                    value,
                });
            }
            Action::AskHasScheduled { pos, block } => {
                let value = cx.has_scheduled_block_tick(pos, block);
                self.push(Entry::Answer {
                    now,
                    query: Query::HasScheduled,
                    pos,
                    value,
                });
            }
            Action::Read { pos } => {
                let result = cx.state(pos);
                self.push(Entry::Read { now, pos, result });
            }
            Action::UpdateNeighbour { target, source } => cx.update_neighbour(target, source),
            Action::UpdateNeighbours { source, block } => cx.update_neighbours(source, block, None),
            Action::UpdateNeighbourWithState {
                target,
                state,
                source,
            } => cx.update_neighbour_with_state(target, state, source, false),
            Action::Mark(label) => self.push(Entry::Mark(label)),
        }
    }
}

impl BlockBehaviour<ProbeKinds> for ProbeBehaviour {
    fn block_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
    ) {
        self.push(Entry::Hook {
            now: cx.now().0,
            hook: Hook::BlockTick,
            pos: at,
            state,
        });
        self.run(cx, Hook::BlockTick, at);
    }

    fn fluid_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _fluid: ProbeFluid,
    ) {
        self.push(Entry::Hook {
            now: cx.now().0,
            hook: Hook::FluidTick,
            pos: at,
            state,
        });
        self.run(cx, Hook::FluidTick, at);
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        self.push(Entry::Hook {
            now: cx.now().0,
            hook: Hook::BlockEvent,
            pos: at,
            state,
        });
        self.run(cx, Hook::BlockEvent, at)
    }

    fn removed<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        old: BlockStateId,
        moved_by_piston: bool,
    ) {
        self.push(Entry::Removed {
            now: cx.now().0,
            pos: at,
            old,
            moved_by_piston,
        });
    }

    fn placed<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        new: BlockStateId,
        old: BlockStateId,
        moved_by_piston: bool,
    ) {
        self.push(Entry::Placed {
            now: cx.now().0,
            pos: at,
            new,
            old,
            moved_by_piston,
        });
        self.run(cx, Hook::Placed, at);
    }

    fn neighbour_changed<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        source: ProbeBlock,
        moved_by_piston: bool,
    ) {
        let depth = self.nesting.fetch_add(1, Ordering::Relaxed) + 1;
        self.deepest.fetch_max(depth, Ordering::Relaxed);
        self.push(Entry::Neighbour {
            pos: at,
            state,
            source,
            moved_by_piston,
        });
        self.run(cx, Hook::Neighbour, at);
        self.nesting.fetch_sub(1, Ordering::Relaxed);
    }

    fn update_shape<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        _cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        from: Direction,
        neighbour: BlockPos,
        neighbour_state: BlockStateId,
    ) -> BlockStateId {
        self.push(Entry::Shape {
            pos: at,
            state,
            from,
            neighbour,
            neighbour_state,
        });
        let supported = from != Direction::Down || !ProbeFacts.is_air(neighbour_state);
        match state {
            STACK if from == Direction::Down => STACK_CHANGED,
            NEEDS_SUPPORT | NEEDS_SUPPORT_WET if !supported => AIR,
            _ => state,
        }
    }
}

pub struct Host {
    volume: SparseVolume,
    overrides: HashMap<SectionPos, Access>,
    properties: HashMap<SectionPos, SectionProperties>,
}

impl Host {
    pub fn new() -> Self {
        Self::within(BoundingBox {
            min: BlockPos::new(-512, -64, -512),
            max: BlockPos::new(511, 319, 511),
        })
    }

    pub fn tall() -> Self {
        Self::within(BoundingBox {
            min: BlockPos::new(-512, -64, -512),
            max: BlockPos::new(1535, 1023, 511),
        })
    }

    fn within(bounds: BoundingBox) -> Self {
        Self {
            volume: SparseVolume::new(bounds),
            overrides: HashMap::new(),
            properties: HashMap::new(),
        }
    }

    pub fn insert(&mut self, at: SectionPos, state: BlockStateId) {
        assert!(self.volume.insert(at, state), "section inside the bounds");
    }

    pub fn put(&mut self, pos: BlockPos, state: BlockStateId) {
        self.volume.set_block(pos, state).expect("loaded section");
    }

    pub fn set_access(&mut self, at: SectionPos, access: Access) {
        self.overrides.insert(at, access);
    }

    pub fn set_properties(&mut self, at: SectionPos, properties: SectionProperties) {
        self.properties.insert(at, properties);
    }

    pub fn state(&self, pos: BlockPos) -> BlockStateId {
        self.volume.block(pos).expect("loaded section")
    }
}

impl Volume for Host {
    fn bounds(&self) -> BoundingBox {
        self.volume.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        let held = self.volume.access(section);
        self.overrides
            .get(&section)
            .map_or(held, |overridden| held.meet(*overridden))
    }
}

impl BlockVolume for Host {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        self.access(at).check_read()?;
        self.volume.section(at)
    }
}

impl BlockVolumeMut for Host {
    type SectionMut<'a> = PalettedSectionMut<'a>;

    fn section_mut(&mut self, at: SectionPos) -> Result<PalettedSectionMut<'_>, Denied> {
        self.access(at).check_write()?;
        self.volume.section_mut(at)
    }
}

impl SimulatedSections for Host {
    fn properties(&self, section: SectionPos) -> SectionProperties {
        self.properties
            .get(&section)
            .copied()
            .unwrap_or(SectionProperties::all())
    }
}

pub struct Runner {
    pub shard: BlockShard<ProbeKinds>,
    pub host: Host,
    pub facts: ProbeFacts,
    pub behaviour: ProbeBehaviour,
    pub config: KernelConfig,
    pub kind: PartitionKind,
    pub partition: ShardId,
}

impl Runner {
    pub fn new(host: Host, rules: Vec<Rule>) -> Self {
        Self {
            shard: BlockShard::new(),
            host,
            facts: ProbeFacts,
            behaviour: ProbeBehaviour::new(rules),
            config: KernelConfig::default(),
            kind: PartitionKind::Shard,
            partition: ShardId(0),
        }
    }

    pub fn step_into(
        &mut self,
        now: i64,
        phase: Phase,
        inbound: &[Message<ProbeKinds>],
        out: &mut StepOutput<ProbeKinds>,
    ) -> Result<(), StepError> {
        self.step_into_with(now, phase, inbound, &[], out)
    }

    pub fn step_into_with(
        &mut self,
        now: i64,
        phase: Phase,
        inbound: &[Message<ProbeKinds>],
        ledgers: &[ChainLedger],
        out: &mut StepOutput<ProbeKinds>,
    ) -> Result<(), StepError> {
        let env = Env {
            partition: self.partition,
            facts: &self.facts,
            behaviour: &self.behaviour,
            policy: &SemanticOrder,
            kind: self.kind,
            config: &self.config,
        };
        let at = StepAt {
            now: GameTick(now),
            phase,
        };
        step(
            &mut self.shard,
            at,
            &mut self.host,
            &env,
            PhaseInput { inbound, ledgers },
            out,
        )
    }

    pub fn step(
        &mut self,
        now: i64,
        phase: Phase,
        inbound: &[Message<ProbeKinds>],
    ) -> Result<StepOutput<ProbeKinds>, StepError> {
        let mut out = StepOutput::default();
        self.step_into(now, phase, inbound, &mut out)?;
        Ok(out)
    }

    pub fn step_with(
        &mut self,
        now: i64,
        phase: Phase,
        inbound: &[Message<ProbeKinds>],
        ledgers: &[ChainLedger],
    ) -> Result<StepOutput<ProbeKinds>, StepError> {
        let mut out = StepOutput::default();
        self.step_into_with(now, phase, inbound, ledgers, &mut out)?;
        Ok(out)
    }

    pub fn level_tick(
        &mut self,
        now: i64,
        inbound: &[Message<ProbeKinds>],
    ) -> StepOutput<ProbeKinds> {
        let mut out = StepOutput::default();
        self.step_into(now, Phase::BlockTicks, inbound, &mut out)
            .expect("inbound messages owned");
        for phase in [Phase::FluidTicks, Phase::BlockEvents] {
            self.step_into(now, phase, &[], &mut out)
                .expect("no inbound messages");
        }
        out
    }
}

pub fn stamp(tick: i64, counter: u32) -> SubTick {
    SubTick::runtime(GameTick(tick), counter)
}

pub const ELSEWHERE: ShardId = ShardId(7);

pub fn inbound(stamp: SubTick, body: MessageBody<ProbeKinds>) -> Message<ProbeKinds> {
    Message {
        stamp,
        origin: ChainOrigin {
            stamp,
            partition: ELSEWHERE,
        },
        body,
    }
}

fn message(counter: u32, body: MessageBody<ProbeKinds>) -> Message<ProbeKinds> {
    inbound(stamp(1, counter), body)
}

pub fn block_tick_message(
    counter: u32,
    pos: BlockPos,
    block: ProbeBlock,
    due: i64,
) -> Message<ProbeKinds> {
    message(
        counter,
        MessageBody::BlockTick {
            pos,
            block,
            due: GameTick(due),
            priority: TickPriority::Normal,
        },
    )
}

pub fn fluid_tick_message(
    counter: u32,
    pos: BlockPos,
    fluid: ProbeFluid,
    due: i64,
) -> Message<ProbeKinds> {
    message(
        counter,
        MessageBody::FluidTick {
            pos,
            fluid,
            due: GameTick(due),
            priority: TickPriority::Normal,
        },
    )
}

pub fn event_message(counter: u32, pos: BlockPos, block: ProbeBlock) -> Message<ProbeKinds> {
    message(
        counter,
        MessageBody::Event(BlockEvent {
            pos,
            block,
            a: 0,
            b: 0,
        }),
    )
}

pub fn set_message(
    counter: u32,
    pos: BlockPos,
    state: BlockStateId,
    flags: u32,
) -> Message<ProbeKinds> {
    limited_set_message(counter, pos, state, flags, 512)
}

pub fn limited_set_message(
    counter: u32,
    pos: BlockPos,
    state: BlockStateId,
    flags: u32,
    limit: u16,
) -> Message<ProbeKinds> {
    message(
        counter,
        MessageBody::Set {
            pos,
            state,
            flags: BlockUpdateFlags::from_bits_retain(flags),
            limit,
        },
    )
}

pub const NEIGHBOUR_ORDER: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
];

pub const SHAPE_ORDER: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
    Direction::Down,
    Direction::Up,
];

pub fn offset(pos: BlockPos, direction: Direction) -> BlockPos {
    let normal = direction.normal();
    at(pos.x + normal.x, pos.y + normal.y, pos.z + normal.z)
}

pub fn around(pos: BlockPos, order: &[Direction; 6]) -> Vec<BlockPos> {
    order.iter().map(|d| offset(pos, *d)).collect()
}

pub fn filled(sections: &[(i32, i32, i32)]) -> Host {
    let mut host = Host::new();
    for (x, y, z) in sections {
        host.insert(section(*x, *y, *z), A);
    }
    host
}

pub fn neighbour_entries(log: &[Entry]) -> Vec<(BlockPos, BlockStateId, ProbeBlock)> {
    log.iter()
        .filter_map(|entry| match entry {
            Entry::Neighbour {
                pos, state, source, ..
            } => Some((*pos, *state, *source)),
            _ => None,
        })
        .collect()
}

pub fn shape_entries(log: &[Entry]) -> Vec<(BlockPos, Direction, BlockPos, BlockStateId)> {
    log.iter()
        .filter_map(|entry| match entry {
            Entry::Shape {
                pos,
                from,
                neighbour,
                neighbour_state,
                ..
            } => Some((*pos, *from, *neighbour, *neighbour_state)),
            _ => None,
        })
        .collect()
}

pub fn neighbour_rule(at: BlockPos, actions: Vec<Action>) -> Rule {
    Rule::on(Hook::Neighbour, at, actions)
}
