// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALL_OF: Id<crate::RuleTestType> = Id::from_static(0);
pub const ALWAYS_TRUE: Id<crate::RuleTestType> = Id::from_static(1);
pub const ANY_OF: Id<crate::RuleTestType> = Id::from_static(2);
pub const BLOCK_MATCH: Id<crate::RuleTestType> = Id::from_static(3);
pub const BLOCKSTATE_MATCH: Id<crate::RuleTestType> = Id::from_static(4);
pub const HEIGHT_MATCH: Id<crate::RuleTestType> = Id::from_static(5);
pub const NOT: Id<crate::RuleTestType> = Id::from_static(6);
pub const RANDOM_BLOCK_MATCH: Id<crate::RuleTestType> = Id::from_static(7);
pub const RANDOM_BLOCKSTATE_MATCH: Id<crate::RuleTestType> = Id::from_static(8);
pub const TAG_MATCH: Id<crate::RuleTestType> = Id::from_static(9);

pub const NAMES: &[&str] = &[
    "minecraft:all_of",
    "minecraft:always_true",
    "minecraft:any_of",
    "minecraft:block_match",
    "minecraft:blockstate_match",
    "minecraft:height_match",
    "minecraft:not",
    "minecraft:random_block_match",
    "minecraft:random_blockstate_match",
    "minecraft:tag_match",
];
