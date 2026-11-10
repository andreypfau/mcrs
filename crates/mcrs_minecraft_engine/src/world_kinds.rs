//! What the block simulation of a dimension ticks: blocks and fluids by their registry ids, and
//! the facts of the block table that the simulation kernels ask for.

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry, BlockStateFlags};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::Direction;
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
    wire: Option<WireLayout>,
}

/// The state-id digits of the four sides of redstone wire, resolved once so that a tick reads a
/// side by arithmetic instead of by property name.
#[derive(Clone, Copy, Debug)]
struct WireLayout {
    base: u16,
    sides: [WireSide; 4],
}

#[derive(Clone, Copy, Debug)]
struct WireSide {
    stride: u16,
    values: u16,
    none: u16,
}

impl WireLayout {
    fn of(entry: &BlockEntry) -> Option<Self> {
        let side = |name: &str| {
            let index = entry.properties.index_of(name)?;
            let (property, later) = entry.properties.0.get(index..)?.split_first()?;
            Some(WireSide {
                stride: later.iter().map(|p| p.values.len() as u16).product(),
                values: u16::try_from(property.values.len()).ok()?,
                none: u16::try_from(property.values.iter().position(|v| v.renders_to("none"))?)
                    .ok()?,
            })
        };
        Some(Self {
            base: entry.base_state_id.number(),
            sides: [side("north")?, side("east")?, side("south")?, side("west")?],
        })
    }

    fn connects(self, state: BlockStateId, side: usize) -> bool {
        let Some(side) = self.sides.get(side) else {
            return false;
        };
        state
            .number()
            .checked_sub(self.base)
            .and_then(|offset| offset.checked_div(side.stride))
            .and_then(|digit| digit.checked_rem(side.values))
            .is_some_and(|digit| digit != side.none)
    }
}

impl<'a> WorldFacts<'a> {
    #[must_use]
    pub fn new(definitions: &'a BlockDefinitions) -> Self {
        Self {
            definitions,
            void_air: definitions.default_state_of(Block::VoidAir.id()),
            wire: definitions
                .entry(Block::RedstoneWire.id())
                .and_then(WireLayout::of),
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
        self.definitions
            .block_index(state)
            .is_some_and(|block| RAILS.iter().any(|rail| rail.id() == block))
    }

    fn is_redstone_wire(&self, state: BlockStateId) -> bool {
        self.is_block(state, Block::RedstoneWire)
    }

    fn wire_connects(&self, state: BlockStateId, direction: Direction) -> bool {
        if !self.is_redstone_wire(state) {
            return false;
        }
        let side = match direction {
            Direction::North => 0,
            Direction::East => 1,
            Direction::South => 2,
            Direction::West => 3,
            Direction::Down | Direction::Up => return false,
        };
        self.wire.is_some_and(|wire| wire.connects(state, side))
    }

    fn is_comparator(&self, state: BlockStateId) -> bool {
        self.is_block(state, Block::Comparator)
    }

    fn is_redstone_conductor(&self, state: BlockStateId) -> bool {
        self.definitions
            .state(state)
            .flags
            .contains(BlockStateFlags::REDSTONE_CONDUCTOR)
    }

    fn has_analog_output(&self, state: BlockStateId) -> bool {
        self.definitions
            .entry(self.block_of(state))
            // chisle: only containers; a block with an analog output and no container (composter,
            // cauldron, cake, lectern) waits for an analog output component in the definitions.
            .is_some_and(|entry| entry.container_slots.is_some())
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

#[cfg(test)]
mod tests {
    use bevy_app::{App, TaskPoolPlugin};
    use bevy_asset::{AssetPlugin, AssetServer};
    use mcrs_minecraft_block::definition::load_block_definitions;

    use super::*;

    #[test]
    fn wire_connects_reads_each_side_of_the_state() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
        let asset_server = app.world().resource::<AssetServer>().clone();
        let (definitions, _) = load_block_definitions(&asset_server).unwrap();
        let facts = WorldFacts::new(&definitions);
        let wire = definitions.entry(Block::RedstoneWire.id()).unwrap();
        let state = [
            ("north", "side"),
            ("east", "none"),
            ("south", "up"),
            ("west", "none"),
        ]
        .into_iter()
        .try_fold(wire.default_state_id, |state, (side, value)| {
            wire.with_text(state, side, value)
        })
        .unwrap();

        let connected = [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
            Direction::Up,
        ]
        .map(|direction| facts.wire_connects(state, direction));

        assert_eq!(connected, [true, false, true, false, false]);
    }
}
