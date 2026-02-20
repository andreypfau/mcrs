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

// World logic — expand as features need them
pub const LOGS: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:logs"));
pub const LEAVES: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:leaves"));
pub const SAND: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:sand"));
pub const WOOL: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:wool"));
pub const SNOW: TagKey<Block> = TagKey::new(mcrs_core::rl!("minecraft:snow"));
