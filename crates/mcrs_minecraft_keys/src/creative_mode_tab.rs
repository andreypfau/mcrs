// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BUILDING_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(0);
pub const COLORED_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(1);
pub const COMBAT: Id<crate::CreativeModeTab> = Id::from_static(8);
pub const FOOD_AND_DRINKS: Id<crate::CreativeModeTab> = Id::from_static(9);
pub const FUNCTIONAL_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(3);
pub const HOTBAR: Id<crate::CreativeModeTab> = Id::from_static(5);
pub const INGREDIENTS: Id<crate::CreativeModeTab> = Id::from_static(10);
pub const INVENTORY: Id<crate::CreativeModeTab> = Id::from_static(13);
pub const NATURAL_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(2);
pub const OP_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(12);
pub const REDSTONE_BLOCKS: Id<crate::CreativeModeTab> = Id::from_static(4);
pub const SEARCH: Id<crate::CreativeModeTab> = Id::from_static(6);
pub const SPAWN_EGGS: Id<crate::CreativeModeTab> = Id::from_static(11);
pub const TOOLS_AND_UTILITIES: Id<crate::CreativeModeTab> = Id::from_static(7);

pub const NAMES: &[&str] = &[
    "minecraft:building_blocks",
    "minecraft:colored_blocks",
    "minecraft:natural_blocks",
    "minecraft:functional_blocks",
    "minecraft:redstone_blocks",
    "minecraft:hotbar",
    "minecraft:search",
    "minecraft:tools_and_utilities",
    "minecraft:combat",
    "minecraft:food_and_drinks",
    "minecraft:ingredients",
    "minecraft:spawn_eggs",
    "minecraft:op_blocks",
    "minecraft:inventory",
];
