// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLACKSTONE_REPLACE: Id<crate::StructureProcessor> = Id::from_static(0);
pub const BLOCK_AGE: Id<crate::StructureProcessor> = Id::from_static(1);
pub const BLOCK_IGNORE: Id<crate::StructureProcessor> = Id::from_static(2);
pub const BLOCK_ROT: Id<crate::StructureProcessor> = Id::from_static(3);
pub const CAPPED: Id<crate::StructureProcessor> = Id::from_static(4);
pub const GRAVITY: Id<crate::StructureProcessor> = Id::from_static(5);
pub const JIGSAW_REPLACEMENT: Id<crate::StructureProcessor> = Id::from_static(6);
pub const LAVA_SUBMERGED_BLOCK: Id<crate::StructureProcessor> = Id::from_static(7);
pub const NOP: Id<crate::StructureProcessor> = Id::from_static(8);
pub const PROTECTED_BLOCKS: Id<crate::StructureProcessor> = Id::from_static(9);
pub const RULE: Id<crate::StructureProcessor> = Id::from_static(10);

pub const NAMES: &[&str] = &[
    "minecraft:blackstone_replace",
    "minecraft:block_age",
    "minecraft:block_ignore",
    "minecraft:block_rot",
    "minecraft:capped",
    "minecraft:gravity",
    "minecraft:jigsaw_replacement",
    "minecraft:lava_submerged_block",
    "minecraft:nop",
    "minecraft:protected_blocks",
    "minecraft:rule",
];
