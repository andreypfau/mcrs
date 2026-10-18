use mcrs_minecraft_core::{BlockPos, BlockStateId, BoundingBox, SectionPos};
use mcrs_minecraft_simulation::SimulationVolume;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, Cx, Env, GameTick,
    KernelConfig, Kinds, Message, MessageBody, PartitionKind, Phase, PhaseInput, SectionProperties,
    SemanticOrder, SimulatedSections, StepAt, StepError, StepOutput, SubTick, TickPriority, step,
};
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, Denied, PalettedSection, PalettedSectionMut, SparseVolume,
    Unavailable, Volume,
};
use std::collections::HashMap;
use std::sync::Mutex;

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

pub const BLOCK_A: ProbeBlock = ProbeBlock(10);
pub const BLOCK_B: ProbeBlock = ProbeBlock(11);
pub const BLOCK_C: ProbeBlock = ProbeBlock(12);
pub const WATER: ProbeFluid = ProbeFluid(1);

const BLOCKS: [u16; STATES] = [0, 1, 10, 10, 11, 12, 13, 20, 21, 30, 30, 0, 0, 0, 0, 0];

pub struct ProbeFacts;

impl BlockFacts<ProbeKinds> for ProbeFacts {
    fn block_of(&self, state: BlockStateId) -> ProbeBlock {
        ProbeBlock(BLOCKS.get(state.index()).copied().unwrap_or(0))
    }

    fn fluid_of(&self, state: BlockStateId) -> Option<ProbeFluid> {
        (state == FLOWING || state == SOURCE).then_some(WATER)
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
}

impl ProbeBehaviour {
    pub fn new(rules: Vec<Rule>) -> Self {
        let fired = Mutex::new(vec![false; rules.len()]);
        Self {
            rules,
            fired,
            log: Mutex::new(Vec::new()),
        }
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
}

pub struct Host {
    volume: SparseVolume,
    overrides: HashMap<SectionPos, Access>,
    properties: HashMap<SectionPos, SectionProperties>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            volume: SparseVolume::new(BoundingBox {
                min: BlockPos::new(-512, -64, -512),
                max: BlockPos::new(511, 319, 511),
            }),
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
        }
    }

    pub fn step_into(
        &mut self,
        now: i64,
        phase: Phase,
        inbound: &[Message<ProbeKinds>],
        out: &mut StepOutput<ProbeKinds>,
    ) -> Result<(), StepError> {
        let env = Env {
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
            PhaseInput { inbound },
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

fn message(counter: u32, body: MessageBody<ProbeKinds>) -> Message<ProbeKinds> {
    let stamp = stamp(1, counter);
    Message {
        stamp,
        origin: stamp,
        body,
    }
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
    message(
        counter,
        MessageBody::Set {
            pos,
            state,
            flags: BlockUpdateFlags::from_bits_retain(flags),
            limit: 512,
        },
    )
}
