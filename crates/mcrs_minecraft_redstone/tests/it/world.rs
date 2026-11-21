use std::sync::LazyLock;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{
    BlockDefinitions, BlockStateFlags, build_block_definitions,
};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::{BoundingBox, Direction};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_redstone::{
    BlockUse, Redstone, RedstoneMemory, RedstoneRules, RedstoneSnapshot, RedstoneTable,
};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockEvent, BlockFacts, BlockShard, BlockUpdateFlags, ChainOrigin, Cx, Env,
    GameTick, KernelConfig, Kinds, Message, MessageBody, PartitionKind, Phase, PhaseInput,
    SectionProperties, ShardId, SimulatedSections, SimulationVolume, StepAt, StepOutput, SubTick,
    UpdateRequest, VanillaOrder, step,
};
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, Denied, PalettedSection, PalettedSectionMut, SparseVolume,
    Unavailable, Volume,
};
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestKinds;

impl Kinds for TestKinds {
    type Block = Id<Block>;
    type Fluid = Id<Fluid>;
}

static DEFINITIONS: LazyLock<BlockDefinitions> = LazyLock::new(|| {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/block_definition"
    );
    let files = std::fs::read_dir(directory)
        .expect("the corpus")
        .map(|entry| entry.expect("a corpus file").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            (
                path.display().to_string(),
                std::fs::read(&path).expect("a corpus file"),
            )
        });
    let names: Vec<&str> = Block::ALL
        .iter()
        .map(|block| block.as_static_str())
        .collect();
    let (definitions, _) = build_block_definitions(&names, files).expect("the corpus");
    definitions
});

static TABLE: LazyLock<RedstoneTable> = LazyLock::new(|| {
    let registries = test_registries();
    let tags = registries.tags::<Block>();
    RedstoneTable::new(&DEFINITIONS, |block, tag| {
        tags.as_ref()
            .is_some_and(|tags| tags.id(tag).is_some_and(|tag| tags.contains(tag, block)))
    })
});

pub fn definitions() -> &'static BlockDefinitions {
    &DEFINITIONS
}

pub fn table() -> &'static RedstoneTable {
    &TABLE
}

pub struct Facts;

impl BlockFacts<TestKinds> for Facts {
    fn block_of(&self, state: BlockStateId) -> Id<Block> {
        definitions().block_index(state).unwrap_or(Block::Air.id())
    }

    fn fluid_of(&self, _state: BlockStateId) -> Option<Id<Fluid>> {
        None
    }

    fn void_air(&self) -> BlockStateId {
        definitions().default_state_of(Block::VoidAir.id())
    }

    fn is_air(&self, state: BlockStateId) -> bool {
        definitions()
            .state(state)
            .flags
            .contains(BlockStateFlags::IS_AIR)
    }

    fn is_rail(&self, _state: BlockStateId) -> bool {
        false
    }

    fn is_redstone_wire(&self, state: BlockStateId) -> bool {
        self.block_of(state) == Block::RedstoneWire.id()
    }

    fn wire_connects(&self, state: BlockStateId, direction: Direction) -> bool {
        self.is_redstone_wire(state) && table().connection(state, direction).is_connected()
    }

    fn is_comparator(&self, state: BlockStateId) -> bool {
        self.block_of(state) == Block::Comparator.id()
    }

    fn is_redstone_conductor(&self, state: BlockStateId) -> bool {
        table().is_conductor(state)
    }

    fn has_analog_output(&self, state: BlockStateId) -> bool {
        definitions()
            .entry(self.block_of(state))
            .is_some_and(|entry| entry.container_slots.is_some())
    }

    fn legacy_block_of_fluid(&self, _state: BlockStateId) -> BlockStateId {
        BlockStateId::AIR
    }
}

pub struct Rig {
    redstone: Redstone,
}

impl BlockBehaviour<TestKinds> for Rig {
    type Memory = RedstoneMemory;

    type Shared = RedstoneSnapshot;

    fn block_tick<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
    ) {
        self.redstone.block_tick(cx, at, state);
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        a: i32,
        b: i32,
    ) -> bool {
        self.redstone.block_event(cx, at, state, a, b)
    }

    fn removed<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        old: BlockStateId,
        moved_by_piston: bool,
    ) {
        self.redstone.removed(cx, at, old, moved_by_piston);
    }

    fn placed<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        new: BlockStateId,
        old: BlockStateId,
        _moved_by_piston: bool,
    ) {
        self.redstone.placed(cx, at, new, old);
    }

    fn neighbour_changed<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _source: Id<Block>,
        _moved_by_piston: bool,
    ) {
        self.redstone.neighbour_changed(cx, at, state);
    }

    fn update_shape<V: SimulationVolume, F: BlockFacts<TestKinds>>(
        &self,
        cx: &mut Cx<'_, TestKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        from: Direction,
        _neighbour: BlockPos,
        neighbour_state: BlockStateId,
    ) -> BlockStateId {
        self.redstone
            .update_shape(cx, at, state, from, neighbour_state)
    }
}

pub struct Host {
    volume: SparseVolume,
}

impl Volume for Host {
    fn bounds(&self) -> BoundingBox {
        self.volume.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        self.volume.access(section)
    }
}

impl BlockVolume for Host {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        self.volume.section(at)
    }
}

impl BlockVolumeMut for Host {
    type SectionMut<'a> = PalettedSectionMut<'a>;

    fn section_mut(&mut self, at: SectionPos) -> Result<Self::SectionMut<'_>, Denied> {
        self.volume.section_mut(at)
    }
}

impl SimulatedSections for Host {
    fn properties(&self, _section: SectionPos) -> SectionProperties {
        SectionProperties::all()
    }
}

pub const FLOOR: i32 = 64;

pub struct World {
    host: Host,
    shard: BlockShard<TestKinds>,
    pub memory: RedstoneMemory,
    pub snapshot: RedstoneSnapshot,
    pub events: Vec<BlockEvent<Id<Block>>>,
    rig: Rig,
    out: StepOutput<TestKinds>,
    pending: Vec<Message<TestKinds>>,
    counter: u32,
    pub now: i64,
}

impl World {
    pub fn new() -> Self {
        Self::with_rules(RedstoneRules::JAVA)
    }

    pub fn with_rules(rules: RedstoneRules) -> Self {
        let mut volume = SparseVolume::new(BoundingBox {
            min: BlockPos::new(-64, 0, -64),
            max: BlockPos::new(63, 127, 63),
        });
        for x in -4..4 {
            for z in -4..4 {
                for y in 0..8 {
                    assert!(
                        volume.insert(SectionPos::new(x, y, z), BlockStateId::AIR),
                        "section inside the bounds"
                    );
                }
            }
        }
        let mut world = Self {
            host: Host { volume },
            shard: BlockShard::new(),
            memory: RedstoneMemory::default(),
            snapshot: RedstoneSnapshot::default(),
            events: Vec::new(),
            rig: Rig {
                redstone: Redstone::new(table().clone(), rules),
            },
            out: StepOutput::default(),
            pending: Vec::new(),
            counter: 0,
            now: 0,
        };
        let stone = world.default("minecraft:stone");
        for x in -24..24 {
            for z in -24..24 {
                world.fill(BlockPos::new(x, FLOOR - 1, z), stone);
            }
        }
        world
    }

    pub fn default(&self, identifier: &str) -> BlockStateId {
        definitions()
            .default_state(identifier)
            .unwrap_or_else(|| panic!("the block {identifier}"))
    }

    pub fn with(&self, state: BlockStateId, property: &str, value: &str) -> BlockStateId {
        let entry = definitions().owner(state).expect("a known state");
        entry
            .with_text(state, property, value)
            .unwrap_or_else(|| panic!("{property}={value} of {}", entry.identifier))
    }

    pub fn value(&self, state: BlockStateId, property: &str) -> String {
        let entry = definitions().owner(state).expect("a known state");
        entry
            .value_of(state, property)
            .map(PropertyValue::to_text)
            .unwrap_or_else(|| panic!("{property} of {}", entry.identifier))
    }

    pub fn facing(&self, state: BlockStateId, direction: Direction) -> BlockStateId {
        self.with(state, "facing", direction.name())
    }

    pub fn fill(&mut self, pos: BlockPos, state: BlockStateId) {
        self.host.set_block(pos, state).expect("a loaded section");
    }

    pub fn state(&self, pos: BlockPos) -> BlockStateId {
        self.host.block(pos).expect("a loaded section")
    }

    pub fn is(&self, pos: BlockPos, identifier: &str) -> bool {
        definitions()
            .owner(self.state(pos))
            .is_some_and(|entry| entry.identifier.as_str() == identifier)
    }

    pub fn place(&mut self, pos: BlockPos, state: BlockStateId) {
        self.request(pos, state, BlockUpdateFlags::ALL);
    }

    pub fn request(&mut self, pos: BlockPos, state: BlockStateId, flags: BlockUpdateFlags) {
        self.send(MessageBody::Set {
            pos,
            state,
            flags,
            limit: 512,
        });
    }

    pub fn request_update(&mut self, request: UpdateRequest<TestKinds>) {
        self.send(MessageBody::Update(request));
    }

    fn send(&mut self, body: MessageBody<TestKinds>) {
        let stamp = SubTick::runtime(GameTick(self.now), self.counter);
        self.counter += 1;
        self.pending.push(Message {
            stamp,
            origin: ChainOrigin {
                stamp,
                partition: ShardId(0),
            },
            body,
        });
    }

    pub fn tick(&mut self) {
        let inbound = std::mem::take(&mut self.pending);
        let config = KernelConfig::default();
        let env = Env {
            partition: ShardId(0),
            facts: &Facts,
            behaviour: &self.rig,
            policy: &VanillaOrder,
            kind: PartitionKind::Shard,
            config: &config,
        };
        let phases = [
            (Phase::BlockTicks, inbound.as_slice()),
            (Phase::FluidTicks, [].as_slice()),
            (Phase::BlockEvents, [].as_slice()),
        ];
        for (phase, messages) in phases {
            let input = PhaseInput {
                inbound: messages,
                ledgers: &[],
            };
            step(
                &mut self.shard,
                StepAt {
                    now: GameTick(self.now),
                    phase,
                },
                &mut self.host,
                &env,
                &mut self.memory,
                &self.snapshot,
                input,
                &mut self.out,
            )
            .expect("a step over owned sections");
        }
        assert_eq!(
            self.out.report.chain_overflows, 0,
            "the chain limit was reached"
        );
        self.events.append(&mut self.out.delivered);
        self.out.clear();
        self.now += 1;
        self.counter = 0;
    }

    pub fn ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.tick();
        }
    }

    pub fn rig_toggle(&self, pos: BlockPos) -> Option<BlockStateId> {
        match self.rig.redstone.used(self.state(pos)) {
            BlockUse::Toggle(toggle) => Some(toggle.state),
            BlockUse::Consumed | BlockUse::Unhandled => None,
        }
    }

    pub fn rig_use(&self, pos: BlockPos) -> BlockUse {
        self.rig.redstone.used(self.state(pos))
    }

    pub fn power(&self, pos: BlockPos) -> u8 {
        table().power(self.state(pos))
    }

    pub fn lit(&self, pos: BlockPos) -> bool {
        table().lit(self.state(pos))
    }

    pub fn powered(&self, pos: BlockPos) -> bool {
        table().powered(self.state(pos))
    }
}

pub fn at(x: i32, z: i32) -> BlockPos {
    BlockPos::new(x, FLOOR, z)
}

pub fn above(pos: BlockPos) -> BlockPos {
    BlockPos::new(pos.x, pos.y + 1, pos.z)
}

pub fn below(pos: BlockPos) -> BlockPos {
    BlockPos::new(pos.x, pos.y - 1, pos.z)
}

pub fn east(pos: BlockPos, count: i32) -> BlockPos {
    BlockPos::new(pos.x + count, pos.y, pos.z)
}

pub fn south(pos: BlockPos, count: i32) -> BlockPos {
    BlockPos::new(pos.x, pos.y, pos.z + count)
}
