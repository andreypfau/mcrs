use crate::Block;
use mcrs_minecraft_core::tag_key::TagKey;

// Tools — used by ToolRule / digging system
pub const MINEABLE_PICKAXE: TagKey<Block> =
    TagKey::new(mcrs_minecraft_core::rl!("minecraft:mineable/pickaxe"));
pub const MINEABLE_AXE: TagKey<Block> =
    TagKey::new(mcrs_minecraft_core::rl!("minecraft:mineable/axe"));

// World logic — expand as features need them
pub const WOOL: TagKey<Block> = TagKey::new(mcrs_minecraft_core::rl!("minecraft:wool"));
pub const SHULKER_BOXES: TagKey<Block> =
    TagKey::new(mcrs_minecraft_core::rl!("minecraft:shulker_boxes"));

// Heightmap predicates
pub const BLOCKS_MOTION_IN_HEIGHTMAP: TagKey<Block> = TagKey::new(mcrs_minecraft_core::rl!(
    "minecraft:blocks_motion_in_heightmap"
));
pub const BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES: TagKey<Block> = TagKey::new(
    mcrs_minecraft_core::rl!("minecraft:blocks_motion_in_heightmap_no_leaves"),
);
