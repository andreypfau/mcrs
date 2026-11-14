use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::BlockStateFlags;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_block::fluid_tag_keys;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::TagKey;
use mcrs_voxel::BlockStateId;

use crate::block_tag_keys::BREAKS_AFTER_FALL;
use crate::block_tag_keys::GRAVITY_BLOCKS;
use crate::concrete_table::ConcreteTable;
use crate::state_flags::StateFlags;

const WATERLOGGED: &str = "waterlogged";

const TAG_FLAGS: [(TagKey<'static, Block>, StateFlags); 4] = [
    (block_tag_keys::FIRE, StateFlags::FREE),
    (GRAVITY_BLOCKS, StateFlags::GRAVITY),
    (BREAKS_AFTER_FALL, StateFlags::BREAKS_AFTER_FALL),
    (block_tag_keys::ANVIL, StateFlags::ANVIL),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateRow {
    flags: StateFlags,
    dry: BlockStateId,
    wet: BlockStateId,
    concrete: Option<BlockStateId>,
}

/// What the rules of a falling block read of each block state, indexed by state id: built once
/// at load from the block definitions, the tags and the concrete pairs, about 10 bytes per
/// state. A state outside the table is not free, does not fall and holds no water.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallingBlockTable {
    rows: Box<[StateRow]>,
    air: BlockStateId,
    water: BlockStateId,
}

impl FallingBlockTable {
    #[must_use]
    pub fn new(
        definitions: &BlockDefinitions,
        block_tags: &Tags<Block>,
        fluid_tags: &Tags<Fluid>,
        concrete: &ConcreteTable,
    ) -> Self {
        let tag_flags: Vec<_> = TAG_FLAGS
            .iter()
            .filter_map(|(key, flag)| Some((block_tags.id(*key)?, *flag)))
            .collect();
        let water_tag = fluid_tags.id(fluid_tag_keys::WATER);
        let rows = definitions
            .states()
            .map(|id| {
                let data = definitions.state(id);
                let block = definitions.block_index(id);
                let holds_water = data.fluid.is_some_and(|fluid| {
                    water_tag.is_some_and(|tag| fluid_tags.contains(tag, fluid.fluid))
                });
                let mut flags = StateFlags::default();
                for (tag, flag) in &tag_flags {
                    flags.set(
                        *flag,
                        block.is_some_and(|block| block_tags.contains(*tag, block)),
                    );
                }
                let is = |key: Block| block == Some(key.id());
                flags.set(
                    StateFlags::FREE,
                    data.flags.contains(BlockStateFlags::REPLACEABLE)
                        || is(Block::Water)
                        || is(Block::Lava),
                );
                flags.set(StateFlags::HOLDS_WATER, holds_water);
                flags.set(
                    StateFlags::WATER_SOURCE,
                    holds_water && data.fluid.is_some_and(|fluid| fluid.source),
                );
                flags.set(StateFlags::MOVING_PISTON, is(Block::MovingPiston));
                let owner = definitions.owner(id);
                let waterlogged = |value| {
                    owner
                        .and_then(|entry| entry.with(id, WATERLOGGED, &PropertyValue::Bool(value)))
                        .unwrap_or(id)
                };
                StateRow {
                    flags,
                    dry: waterlogged(false),
                    wet: waterlogged(true),
                    concrete: block
                        .and_then(|block| concrete.concrete(block))
                        .map(|block| definitions.default_state_of(block)),
                }
            })
            .collect();
        Self {
            rows,
            air: definitions.default_state_of(Block::Air.id()),
            water: definitions.default_state_of(Block::Water.id()),
        }
    }

    /// The bytes that the table holds.
    #[must_use]
    pub fn table_bytes(&self) -> usize {
        size_of_val(&*self.rows)
    }

    /// True when a gravity block above `state` falls (FBK-TRG-004).
    #[must_use]
    pub fn is_free(&self, state: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::FREE)
    }

    /// True when a scheduled tick of `state` with `below` under it starts a fall (FBK-TRG-004).
    #[must_use]
    pub fn falls(&self, state: BlockStateId, below: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::GRAVITY) && self.is_free(below)
    }

    /// True when the fluid of `state` is in the fluid tag `minecraft:water` (FBK-TCK-003,
    /// FBK-LND-004).
    #[must_use]
    pub fn holds_water(&self, state: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::HOLDS_WATER)
    }

    /// True when the block of `state` is a key of the concrete pairs: its falling block looks for
    /// water (FBK-TCK-003).
    #[must_use]
    pub fn is_concrete_powder(&self, state: BlockStateId) -> bool {
        self.row(state).is_some_and(|row| row.concrete.is_some())
    }

    pub(crate) fn flags(&self, state: BlockStateId) -> StateFlags {
        self.row(state).map(|row| row.flags).unwrap_or_default()
    }

    /// `state` with `waterlogged` false, or `state` when it has no such property.
    pub(crate) fn dry(&self, state: BlockStateId) -> BlockStateId {
        self.row(state).map_or(state, |row| row.dry)
    }

    /// `state` with `waterlogged` true, or `state` when it has no such property.
    pub(crate) fn wet(&self, state: BlockStateId) -> BlockStateId {
        self.row(state).map_or(state, |row| row.wet)
    }

    /// The default state of the concrete of the block of `state`.
    pub(crate) fn concrete(&self, state: BlockStateId) -> Option<BlockStateId> {
        self.row(state).and_then(|row| row.concrete)
    }

    /// The block of the fluid of `state`: `water` when it holds water, else `air` (FBK-SPN-001).
    pub(crate) fn leftover(&self, state: BlockStateId) -> BlockStateId {
        if self.holds_water(state) {
            self.water
        } else {
            self.air
        }
    }

    fn row(&self, state: BlockStateId) -> Option<&StateRow> {
        self.rows.get(state.index())
    }
}
