// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const BUILDING_BLOCKS: ResourceLocation<'static> = rl!("minecraft:building_blocks");
pub const COLORED_BLOCKS: ResourceLocation<'static> = rl!("minecraft:colored_blocks");
pub const NATURAL_BLOCKS: ResourceLocation<'static> = rl!("minecraft:natural_blocks");
pub const FUNCTIONAL_BLOCKS: ResourceLocation<'static> = rl!("minecraft:functional_blocks");
pub const REDSTONE_BLOCKS: ResourceLocation<'static> = rl!("minecraft:redstone_blocks");
pub const HOTBAR: ResourceLocation<'static> = rl!("minecraft:hotbar");
pub const SEARCH: ResourceLocation<'static> = rl!("minecraft:search");
pub const TOOLS_AND_UTILITIES: ResourceLocation<'static> = rl!("minecraft:tools_and_utilities");
pub const COMBAT: ResourceLocation<'static> = rl!("minecraft:combat");
pub const FOOD_AND_DRINKS: ResourceLocation<'static> = rl!("minecraft:food_and_drinks");
pub const INGREDIENTS: ResourceLocation<'static> = rl!("minecraft:ingredients");
pub const SPAWN_EGGS: ResourceLocation<'static> = rl!("minecraft:spawn_eggs");
pub const OP_BLOCKS: ResourceLocation<'static> = rl!("minecraft:op_blocks");
pub const INVENTORY: ResourceLocation<'static> = rl!("minecraft:inventory");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
