//! What the block simulation of a dimension ticks: blocks and fluids by their registry ids, and
//! the facts of the block table that the simulation kernels ask for.

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{BlockFacts, Kinds};
use mcrs_voxel::BlockStateId;

/// A scheduled tick of a dimension names a block or a fluid by its registry id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldKinds;

impl Kinds for WorldKinds {
    type Block = Id<Block>;
    type Fluid = Id<Fluid>;
}

const RAILS: [Block; 4] = [
    Block::Rail,
    Block::PoweredRail,
    Block::DetectorRail,
    Block::ActivatorRail,
];

/// The facts of one block table.
#[derive(Clone, Copy, Debug)]
pub struct WorldFacts<'a> {
    definitions: &'a BlockDefinitions,
    void_air: BlockStateId,
}

impl<'a> WorldFacts<'a> {
    #[must_use]
    pub fn new(definitions: &'a BlockDefinitions) -> Self {
        Self {
            definitions,
            void_air: definitions.default_state_of(Block::VoidAir.id()),
        }
    }

    fn is_block(&self, state: BlockStateId, block: Block) -> bool {
        self.definitions.block_index(state) == Some(block.id())
    }
}

impl BlockFacts<WorldKinds> for WorldFacts<'_> {
    fn block_of(&self, state: BlockStateId) -> Id<Block> {
        self.definitions
            .block_index(state)
            .unwrap_or(Block::Air.id())
    }

    fn fluid_of(&self, state: BlockStateId) -> Option<Id<Fluid>> {
        self.definitions
            .state(state)
            .fluid
            .map(|fluid| fluid.fluid)
            .filter(|fluid| *fluid != Fluid::Empty.id())
    }

    fn void_air(&self) -> BlockStateId {
        self.void_air
    }

    fn is_air(&self, state: BlockStateId) -> bool {
        self.definitions
            .state(state)
            .flags
            .contains(BlockStateFlags::IS_AIR)
    }

    fn is_rail(&self, state: BlockStateId) -> bool {
        RAILS.iter().any(|rail| self.is_block(state, *rail))
    }

    fn is_redstone_wire(&self, state: BlockStateId) -> bool {
        self.is_block(state, Block::RedstoneWire)
    }

    /// The block of a fluid on its own: a source is level 0 of the fluid block, and a flowing
    /// fluid of amount `n` is level `8 - n`. The table states no falling fluid, so none is made.
    fn legacy_block_of_fluid(&self, state: BlockStateId) -> BlockStateId {
        let Some(fluid) = self.definitions.state(state).fluid else {
            return BlockStateId::AIR;
        };
        let block = if fluid.fluid == Fluid::Water.id() {
            Block::Water
        } else if fluid.fluid == Fluid::Lava.id() {
            Block::Lava
        } else {
            return BlockStateId::AIR;
        };
        let Some(entry) = self.definitions.entry(block.id()) else {
            return BlockStateId::AIR;
        };
        let level = if fluid.source {
            0
        } else {
            8_u8.saturating_sub(fluid.level).max(1)
        };
        entry
            .with(
                entry.default_state_id,
                "level",
                &PropertyValue::Int(i32::from(level)),
            )
            .unwrap_or(entry.default_state_id)
    }
}
