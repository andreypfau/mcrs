use crate::world;
use crate::world::block::Block;
use world::block::minecraft;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    Tag(&'static Block),
    TagSet(BlockTagSet),
}

pub trait BlockTagSetExt {
    fn contains_block(&self, block: &Block) -> bool;
}

impl BlockTagSetExt for BlockTag {
    fn contains_block(&self, block: &Block) -> bool {
        match self {
            BlockTag::Tag(b) => b == &block,
            BlockTag::TagSet(tag_set) => tag_set.contains_block(block),
        }
    }
}

impl BlockTagSetExt for BlockTagSet {
    fn contains_block(&self, block: &Block) -> bool {
        for tag in *self {
            if tag.contains_block(block) {
                return true;
            }
        }
        false
    }
}

pub const MINEABLE_PICKAXE: BlockTagSet = &[&BlockTag::Tag(&minecraft::STONE)];

pub const MINEABLE_SHOVEL: BlockTagSet = &[
    &BlockTag::Tag(&minecraft::GRASS_BLOCK),
    &BlockTag::Tag(&minecraft::DIRT),
];

pub const NEEDS_DIAMOND_TOOL: BlockTagSet = &[];
pub const NEEDS_IRON_TOOL: BlockTagSet = &[];
pub const NEEDS_STONE_TOOL: BlockTagSet = &[];

pub const INCORRECT_FOR_NETHERITE_TOOL: BlockTagSet = &[];
pub const INCORRECT_FOR_DIAMOND_TOOL: BlockTagSet = &[];
pub const INCORRECT_FOR_IRON_TOOL: BlockTagSet = &[&BlockTag::TagSet(NEEDS_DIAMOND_TOOL)];
pub const INCORRECT_FOR_COPPER_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
];
pub const INCORRECT_FOR_STONE_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
];
pub const INCORRECT_FOR_GOLD_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
    &BlockTag::TagSet(NEEDS_STONE_TOOL),
];
pub const INCORRECT_FOR_WOODEN_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
    &BlockTag::TagSet(NEEDS_STONE_TOOL),
];
