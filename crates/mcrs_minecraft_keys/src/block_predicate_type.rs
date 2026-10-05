// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALL_OF: Id<crate::BlockPredicateType> = Id::from_static(10);
pub const ANY_OF: Id<crate::BlockPredicateType> = Id::from_static(9);
pub const BELOW_HEIGHTMAP: Id<crate::BlockPredicateType> = Id::from_static(16);
pub const HAS_STURDY_FACE: Id<crate::BlockPredicateType> = Id::from_static(4);
pub const HEIGHT_RANGE: Id<crate::BlockPredicateType> = Id::from_static(14);
pub const INSIDE_WORLD_BOUNDS: Id<crate::BlockPredicateType> = Id::from_static(8);
pub const MATCHING_BIOMES: Id<crate::BlockPredicateType> = Id::from_static(3);
pub const MATCHING_BLOCK_TAG: Id<crate::BlockPredicateType> = Id::from_static(1);
pub const MATCHING_BLOCKS: Id<crate::BlockPredicateType> = Id::from_static(0);
pub const MATCHING_FLUIDS: Id<crate::BlockPredicateType> = Id::from_static(2);
pub const NOT: Id<crate::BlockPredicateType> = Id::from_static(11);
pub const REPLACEABLE: Id<crate::BlockPredicateType> = Id::from_static(6);
pub const SOLID: Id<crate::BlockPredicateType> = Id::from_static(5);
pub const TRUE: Id<crate::BlockPredicateType> = Id::from_static(12);
pub const UNOBSTRUCTED: Id<crate::BlockPredicateType> = Id::from_static(13);
pub const VOLUME_MATCH: Id<crate::BlockPredicateType> = Id::from_static(15);
pub const WOULD_SURVIVE: Id<crate::BlockPredicateType> = Id::from_static(7);

pub const NAMES: &[&str] = &[
    "minecraft:matching_blocks",
    "minecraft:matching_block_tag",
    "minecraft:matching_fluids",
    "minecraft:matching_biomes",
    "minecraft:has_sturdy_face",
    "minecraft:solid",
    "minecraft:replaceable",
    "minecraft:would_survive",
    "minecraft:inside_world_bounds",
    "minecraft:any_of",
    "minecraft:all_of",
    "minecraft:not",
    "minecraft:true",
    "minecraft:unobstructed",
    "minecraft:height_range",
    "minecraft:volume_match",
    "minecraft:below_heightmap",
];
