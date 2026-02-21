use crate::block::Block;
use mcrs_core::tag::key::TagKey;

// Tools — used by ToolRule / digging system
pub const MINEABLE_PICKAXE: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:mineable/pickaxe"));
pub const MINEABLE_AXE: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:mineable/axe"));
pub const MINEABLE_SHOVEL: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:mineable/shovel"));
pub const MINEABLE_HOE: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:mineable/hoe"));
pub const NEEDS_CORRECT_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:needs_correct_tool_for_drops"));

// incorrect_for_* — used by ToolMaterial to deny drops
pub const INCORRECT_FOR_WOODEN_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_wooden_tool"));
pub const INCORRECT_FOR_STONE_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_stone_tool"));
pub const INCORRECT_FOR_COPPER_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_copper_tool"));
pub const INCORRECT_FOR_IRON_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_iron_tool"));
pub const INCORRECT_FOR_DIAMOND_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_diamond_tool"));
pub const INCORRECT_FOR_GOLD_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_gold_tool"));
pub const INCORRECT_FOR_NETHERITE_TOOL: TagKey<Block> =
    TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_netherite_tool"));

// World logic — expand as features need them
pub const LOGS: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:logs"));
pub const LEAVES: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:leaves"));
pub const SAND: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:sand"));
pub const WOOL: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:wool"));
pub const SNOW: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:snow"));

/// All block tag keys, for bulk loading.
pub const ALL_BLOCK_TAGS: &[TagKey<Block>] = &[
    MINEABLE_PICKAXE,
    MINEABLE_AXE,
    MINEABLE_SHOVEL,
    MINEABLE_HOE,
    NEEDS_CORRECT_TOOL,
    INCORRECT_FOR_WOODEN_TOOL,
    INCORRECT_FOR_STONE_TOOL,
    INCORRECT_FOR_COPPER_TOOL,
    INCORRECT_FOR_IRON_TOOL,
    INCORRECT_FOR_DIAMOND_TOOL,
    INCORRECT_FOR_GOLD_TOOL,
    INCORRECT_FOR_NETHERITE_TOOL,
    LOGS,
    LEAVES,
    SAND,
    WOOL,
    SNOW,
];
