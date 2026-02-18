use crate::block::Block;
use mcrs_core::tag::key::TagKey;

// Tools — used by ToolRule / digging system
pub const MINEABLE_PICKAXE: TagKey<Block> = TagKey::of("minecraft", "mineable/pickaxe");
pub const MINEABLE_AXE: TagKey<Block> = TagKey::of("minecraft", "mineable/axe");
pub const MINEABLE_SHOVEL: TagKey<Block> = TagKey::of("minecraft", "mineable/shovel");
pub const MINEABLE_HOE: TagKey<Block> = TagKey::of("minecraft", "mineable/hoe");
pub const NEEDS_CORRECT_TOOL: TagKey<Block> =
    TagKey::of("minecraft", "needs_correct_tool_for_drops");

// World logic — expand as features need them
pub const LOGS: TagKey<Block> = TagKey::of("minecraft", "logs");
pub const LEAVES: TagKey<Block> = TagKey::of("minecraft", "leaves");
pub const SAND: TagKey<Block> = TagKey::of("minecraft", "sand");
pub const WOOL: TagKey<Block> = TagKey::of("minecraft", "wool");
pub const SNOW: TagKey<Block> = TagKey::of("minecraft", "snow");
