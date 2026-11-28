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

use crate::concrete_table::ConcreteTable;
use crate::gravity_tags::BREAKS_AFTER_FALL;
use crate::gravity_tags::GRAVITY_BLOCKS;
use crate::state_flags::StateFlags;

const WATERLOGGED: &str = "waterlogged";

#[derive(Clone, Copy, Debug)]
struct TagFlag<T> {
    tag: T,
    flag: StateFlags,
}

const TAG_FLAGS: [TagFlag<TagKey<'static, Block>>; 4] = [
    TagFlag {
        tag: block_tag_keys::FIRE,
        flag: StateFlags::FREE,
    },
    TagFlag {
        tag: GRAVITY_BLOCKS,
        flag: StateFlags::GRAVITY,
    },
    TagFlag {
        tag: BREAKS_AFTER_FALL,
        flag: StateFlags::BREAKS_AFTER_FALL,
    },
    TagFlag {
        tag: block_tag_keys::ANVIL,
        flag: StateFlags::ANVIL,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateRow {
    flags: StateFlags,
    dry: BlockStateId,
    wet: BlockStateId,
    concrete: Option<BlockStateId>,
}

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
        let tag_flags: Vec<TagFlag<_>> = TAG_FLAGS
            .iter()
            .filter_map(|tag_flag| {
                Some(TagFlag {
                    tag: block_tags.id(tag_flag.tag)?,
                    flag: tag_flag.flag,
                })
            })
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
                for tag_flag in &tag_flags {
                    flags.insert_if(
                        tag_flag.flag,
                        block.is_some_and(|block| block_tags.contains(tag_flag.tag, block)),
                    );
                }
                let is = |key: Block| block == Some(key.id());
                flags.insert_if(
                    StateFlags::FREE,
                    data.flags.contains(BlockStateFlags::REPLACEABLE)
                        || is(Block::Water)
                        || is(Block::Lava),
                );
                flags.insert_if(StateFlags::HOLDS_WATER, holds_water);
                flags.insert_if(
                    StateFlags::WATER_SOURCE,
                    holds_water && data.fluid.is_some_and(|fluid| fluid.source),
                );
                flags.insert_if(StateFlags::MOVING_PISTON, is(Block::MovingPiston));
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

    #[must_use]
    pub fn is_free(&self, state: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::FREE)
    }

    #[must_use]
    pub fn falls(&self, state: BlockStateId, below: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::GRAVITY) && self.is_free(below)
    }

    #[must_use]
    pub fn holds_water(&self, state: BlockStateId) -> bool {
        self.flags(state).contains(StateFlags::HOLDS_WATER)
    }

    #[must_use]
    pub fn is_concrete_powder(&self, state: BlockStateId) -> bool {
        self.row(state).is_some_and(|row| row.concrete.is_some())
    }

    pub(crate) fn flags(&self, state: BlockStateId) -> StateFlags {
        self.row(state).map(|row| row.flags).unwrap_or_default()
    }

    pub(crate) fn dry(&self, state: BlockStateId) -> BlockStateId {
        self.row(state).map_or(state, |row| row.dry)
    }

    pub(crate) fn wet(&self, state: BlockStateId) -> BlockStateId {
        self.row(state).map_or(state, |row| row.wet)
    }

    pub(crate) fn concrete(&self, state: BlockStateId) -> Option<BlockStateId> {
        self.row(state).and_then(|row| row.concrete)
    }

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
