// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLASTING: Id<crate::RecipeType> = Id::from_static(2);
pub const BREWING: Id<crate::RecipeType> = Id::from_static(7);
pub const CAMPFIRE_COOKING: Id<crate::RecipeType> = Id::from_static(4);
pub const CRAFTING: Id<crate::RecipeType> = Id::from_static(0);
pub const SMELTING: Id<crate::RecipeType> = Id::from_static(1);
pub const SMITHING: Id<crate::RecipeType> = Id::from_static(6);
pub const SMOKING: Id<crate::RecipeType> = Id::from_static(3);
pub const STONECUTTING: Id<crate::RecipeType> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:crafting",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing",
    "minecraft:brewing",
];
