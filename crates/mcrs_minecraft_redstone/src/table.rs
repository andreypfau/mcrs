use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::placement_filter::FaceTest;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags, ShapeId};
use mcrs_minecraft_block::material::PushReaction;
use mcrs_minecraft_block_support::face_passes;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_resource::{Id, TagKey};
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

use crate::component::{Connection, DiodeMode, Face, Kind, PlateKind, Stickiness};
use crate::layout::BlockLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct StateInfo {
    pub kind: Option<Kind>,
    pub container: bool,
    pub block: Id<Block>,
    pub conductor: bool,
    pub signal_source: bool,
    pub block_entity: bool,
    pub unbreakable: bool,
    pub air: bool,
    pub push: PushReaction,
    pub producer: u8,
    pub stickiness: Stickiness,
    pub faces: SupportFaces,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SupportFaces {
    pub center_up: bool,
    pub rigid_up: bool,
    full: u8,
}

impl SupportFaces {
    fn of(shape: &[Aabb]) -> Self {
        let mut full = 0_u8;
        for direction in Direction::all() {
            if face_passes(shape, direction, FaceTest::Full) {
                full |= 1_u8 << direction.id();
            }
        }
        Self {
            center_up: face_passes(shape, Direction::Up, FaceTest::Center),
            rigid_up: face_passes(shape, Direction::Up, FaceTest::Ring),
            full,
        }
    }

    #[must_use]
    pub fn is_full(self, direction: Direction) -> bool {
        self.full & (1_u8 << direction.id()) != 0
    }
}

#[derive(Clone, Debug)]
pub struct RedstoneTable {
    states: Box<[StateInfo]>,
    layouts: Box<[Option<BlockLayout>]>,
    air: BlockStateId,
    void_air: BlockStateId,
    piston_head: Option<BlockStateId>,
    moving_piston: Option<BlockStateId>,
    piston: Option<BlockStateId>,
    sticky_piston: Option<BlockStateId>,
}

fn kind_of(
    block: Id<Block>,
    tagged: &impl Fn(Id<Block>, TagKey<'static, Block>) -> bool,
) -> Option<Kind> {
    let is = |candidate: Block| candidate.id() == block;
    let kind = if is(Block::RedstoneWire) {
        Kind::Wire
    } else if is(Block::Lever) {
        Kind::Lever
    } else if is(Block::RedstoneTorch) {
        Kind::Torch { wall: false }
    } else if is(Block::RedstoneWallTorch) {
        Kind::Torch { wall: true }
    } else if is(Block::RedstoneBlock) {
        Kind::RedstoneBlock
    } else if is(Block::RedstoneLamp) {
        Kind::Lamp
    } else if is(Block::Repeater) {
        Kind::Repeater
    } else if is(Block::Comparator) {
        Kind::Comparator
    } else if is(Block::Observer) {
        Kind::Observer
    } else if is(Block::Piston) {
        Kind::Piston { sticky: false }
    } else if is(Block::StickyPiston) {
        Kind::Piston { sticky: true }
    } else if is(Block::PistonHead) {
        Kind::PistonHead
    } else if is(Block::MovingPiston) {
        Kind::MovingPiston
    } else if is(Block::Target) {
        Kind::Target
    } else if is(Block::LightWeightedPressurePlate) {
        Kind::Plate(PlateKind::LightWeighted)
    } else if is(Block::HeavyWeightedPressurePlate) {
        Kind::Plate(PlateKind::HeavyWeighted)
    } else if tagged(block, block_tag_keys::STONE_BUTTONS) {
        Kind::Button { wooden: false }
    } else if tagged(block, block_tag_keys::WOODEN_BUTTONS) {
        Kind::Button { wooden: true }
    } else if tagged(block, block_tag_keys::BUTTONS) {
        Kind::Button { wooden: false }
    } else if tagged(block, block_tag_keys::STONE_PRESSURE_PLATES) {
        Kind::Plate(PlateKind::Stone)
    } else if tagged(block, block_tag_keys::PRESSURE_PLATES) {
        Kind::Plate(PlateKind::Wooden)
    } else {
        return None;
    };
    Some(kind)
}

fn stickiness_of(block: Id<Block>) -> Stickiness {
    if block == Block::SlimeBlock.id() {
        Stickiness::Slime
    } else if block == Block::HoneyBlock.id() {
        Stickiness::Honey
    } else {
        Stickiness::None
    }
}

impl RedstoneTable {
    #[must_use]
    pub fn new(
        definitions: &BlockDefinitions,
        tagged: impl Fn(Id<Block>, TagKey<'static, Block>) -> bool,
    ) -> Self {
        let air = definitions.default_state_of(Block::Air.id());
        let void_air = definitions.default_state_of(Block::VoidAir.id());
        let mut faces_of: FxHashMap<ShapeId, SupportFaces> = FxHashMap::default();
        let states = definitions
            .states()
            .map(|state| {
                let data = definitions.state(state);
                let block = definitions.block_index(state).unwrap_or(Block::Air.id());
                let shape = definitions.support_shape(state);
                let faces = *faces_of
                    .entry(shape)
                    .or_insert_with(|| SupportFaces::of(definitions.shape(shape)));
                StateInfo {
                    kind: kind_of(block, &tagged),
                    container: definitions
                        .entry(block)
                        .is_some_and(|entry| entry.container_slots.is_some()),
                    block,
                    conductor: data.flags.contains(BlockStateFlags::REDSTONE_CONDUCTOR),
                    signal_source: data.flags.contains(BlockStateFlags::IS_SIGNAL_SOURCE),
                    block_entity: data.flags.contains(BlockStateFlags::HAS_BLOCK_ENTITY),
                    unbreakable: data.hardness < 0.0,
                    air: data.flags.contains(BlockStateFlags::IS_AIR),
                    push: data.push_reaction,
                    producer: data.redstone_power,
                    stickiness: stickiness_of(block),
                    faces,
                }
            })
            .collect();
        let layouts = definitions
            .blocks()
            .iter()
            .map(|entry| {
                definitions
                    .block_index(entry.default_state_id)
                    .and_then(|block| kind_of(block, &tagged))
                    .map(|_| BlockLayout::of(entry))
            })
            .collect();
        let default_of = |block: Block| {
            definitions
                .entry(block.id())
                .map(|entry| entry.default_state_id)
        };
        Self {
            states,
            layouts,
            air,
            void_air,
            piston_head: default_of(Block::PistonHead),
            moving_piston: default_of(Block::MovingPiston),
            piston: default_of(Block::Piston),
            sticky_piston: default_of(Block::StickyPiston),
        }
    }

    #[must_use]
    pub fn info(&self, state: BlockStateId) -> StateInfo {
        self.states
            .get(state.index())
            .copied()
            .unwrap_or(StateInfo {
                kind: None,
                container: false,
                block: Block::Air.id(),
                conductor: false,
                signal_source: false,
                block_entity: false,
                unbreakable: false,
                air: true,
                push: PushReaction::Normal,
                producer: 0,
                stickiness: Stickiness::None,
                faces: SupportFaces::default(),
            })
    }

    #[must_use]
    pub fn holds_wire(&self, state: BlockStateId) -> bool {
        let info = self.info(state);
        info.faces.center_up || info.block == Block::Hopper.id()
    }

    #[must_use]
    pub fn rigid_top(&self, state: BlockStateId) -> bool {
        self.info(state).faces.rigid_up
    }

    #[must_use]
    pub fn full_face(&self, state: BlockStateId, direction: Direction) -> bool {
        self.info(state).faces.is_full(direction)
    }

    #[must_use]
    pub fn kind(&self, state: BlockStateId) -> Option<Kind> {
        self.info(state).kind
    }

    #[must_use]
    pub fn block_of(&self, state: BlockStateId) -> Id<Block> {
        self.info(state).block
    }

    #[must_use]
    pub fn is_conductor(&self, state: BlockStateId) -> bool {
        self.info(state).conductor
    }

    #[must_use]
    pub fn is_air(&self, state: BlockStateId) -> bool {
        self.info(state).air
    }

    #[must_use]
    pub fn air(&self) -> BlockStateId {
        self.air
    }

    #[must_use]
    pub fn void_air(&self) -> BlockStateId {
        self.void_air
    }

    fn layout(&self, state: BlockStateId) -> Option<&BlockLayout> {
        self.layouts.get(self.info(state).block.index())?.as_ref()
    }

    #[must_use]
    pub fn powered(&self, state: BlockStateId) -> bool {
        self.layout(state)
            .and_then(|layout| layout.read(layout.powered.as_ref(), state))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn lit(&self, state: BlockStateId) -> bool {
        self.layout(state)
            .and_then(|layout| layout.read(layout.lit.as_ref(), state))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn locked(&self, state: BlockStateId) -> bool {
        self.layout(state)
            .and_then(|layout| layout.read(layout.locked.as_ref(), state))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn extended(&self, state: BlockStateId) -> bool {
        self.layout(state)
            .and_then(|layout| layout.read(layout.extended.as_ref(), state))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn power(&self, state: BlockStateId) -> u8 {
        self.layout(state)
            .and_then(|layout| layout.read(layout.power.as_ref(), state))
            .unwrap_or(0)
    }

    #[must_use]
    pub fn delay(&self, state: BlockStateId) -> u8 {
        self.layout(state)
            .and_then(|layout| layout.read(layout.delay.as_ref(), state))
            .unwrap_or(1)
    }

    #[must_use]
    pub fn facing(&self, state: BlockStateId) -> Option<Direction> {
        self.layout(state)
            .and_then(|layout| layout.read(layout.facing.as_ref(), state))
    }

    #[must_use]
    pub fn face(&self, state: BlockStateId) -> Option<Face> {
        self.layout(state)
            .and_then(|layout| layout.read(layout.face.as_ref(), state))
    }

    #[must_use]
    pub fn mode(&self, state: BlockStateId) -> DiodeMode {
        self.layout(state)
            .and_then(|layout| layout.read(layout.mode.as_ref(), state))
            .unwrap_or(DiodeMode::Compare)
    }

    #[must_use]
    pub fn is_sticky_type(&self, state: BlockStateId) -> bool {
        self.layout(state)
            .and_then(|layout| layout.read(layout.sticky.as_ref(), state))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn connection(&self, state: BlockStateId, direction: Direction) -> Connection {
        self.layout(state)
            .and_then(|layout| layout.read(layout.side(direction), state))
            .unwrap_or(Connection::None)
    }

    #[must_use]
    pub fn with_powered(&self, state: BlockStateId, value: bool) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.powered.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_lit(&self, state: BlockStateId, value: bool) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.lit.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_locked(&self, state: BlockStateId, value: bool) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.locked.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_extended(&self, state: BlockStateId, value: bool) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.extended.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_power(&self, state: BlockStateId, value: u8) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.power.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_delay(&self, state: BlockStateId, value: u8) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.delay.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_mode(&self, state: BlockStateId, value: DiodeMode) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.mode.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_facing(&self, state: BlockStateId, value: Direction) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.facing.as_ref(), state, value)
    }

    #[must_use]
    pub fn with_connection(
        &self,
        state: BlockStateId,
        direction: Direction,
        value: Connection,
    ) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.side(direction), state, value)
    }

    #[must_use]
    pub fn with_connections(
        &self,
        state: BlockStateId,
        connections: [Connection; 4],
    ) -> Option<BlockStateId> {
        Direction::HORIZONTAL
            .into_iter()
            .zip(connections)
            .try_fold(state, |state, (direction, connection)| {
                self.with_connection(state, direction, connection)
            })
    }

    fn with_sticky_type(&self, state: BlockStateId, sticky: bool) -> Option<BlockStateId> {
        let layout = self.layout(state)?;
        layout.write(layout.sticky.as_ref(), state, sticky)
    }

    #[must_use]
    pub fn piston_head(&self, facing: Direction, sticky: bool) -> Option<BlockStateId> {
        let head = self.with_facing(self.piston_head?, facing)?;
        let head = self.with_sticky_type(head, sticky)?;
        let layout = self.layout(head)?;
        layout.write(layout.short.as_ref(), head, false)
    }

    #[must_use]
    pub fn moving_piston(&self, facing: Direction, sticky: bool) -> Option<BlockStateId> {
        let moving = self.with_facing(self.moving_piston?, facing)?;
        self.with_sticky_type(moving, sticky)
    }

    #[must_use]
    pub fn piston(&self, sticky: bool, facing: Direction, extended: bool) -> Option<BlockStateId> {
        let base = if sticky {
            self.sticky_piston?
        } else {
            self.piston?
        };
        let piston = self.with_facing(base, facing)?;
        self.with_extended(piston, extended)
    }
}
