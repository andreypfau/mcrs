// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BUILDING_BLOCKS: StaticResourceLocation = rl!("minecraft:building_blocks");
pub const COLORED_BLOCKS: StaticResourceLocation = rl!("minecraft:colored_blocks");
pub const NATURAL_BLOCKS: StaticResourceLocation = rl!("minecraft:natural_blocks");
pub const FUNCTIONAL_BLOCKS: StaticResourceLocation = rl!("minecraft:functional_blocks");
pub const REDSTONE_BLOCKS: StaticResourceLocation = rl!("minecraft:redstone_blocks");
pub const HOTBAR: StaticResourceLocation = rl!("minecraft:hotbar");
pub const SEARCH: StaticResourceLocation = rl!("minecraft:search");
pub const TOOLS_AND_UTILITIES: StaticResourceLocation = rl!("minecraft:tools_and_utilities");
pub const COMBAT: StaticResourceLocation = rl!("minecraft:combat");
pub const FOOD_AND_DRINKS: StaticResourceLocation = rl!("minecraft:food_and_drinks");
pub const INGREDIENTS: StaticResourceLocation = rl!("minecraft:ingredients");
pub const SPAWN_EGGS: StaticResourceLocation = rl!("minecraft:spawn_eggs");
pub const OP_BLOCKS: StaticResourceLocation = rl!("minecraft:op_blocks");
pub const INVENTORY: StaticResourceLocation = rl!("minecraft:inventory");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BUILDING_BLOCKS,
    COLORED_BLOCKS,
    NATURAL_BLOCKS,
    FUNCTIONAL_BLOCKS,
    REDSTONE_BLOCKS,
    HOTBAR,
    SEARCH,
    TOOLS_AND_UTILITIES,
    COMBAT,
    FOOD_AND_DRINKS,
    INGREDIENTS,
    SPAWN_EGGS,
    OP_BLOCKS,
    INVENTORY,
];
