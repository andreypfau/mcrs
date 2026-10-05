// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CRAFTING_SHAPED: Id<crate::RecipeDisplay> = Id::from_static(1);
pub const CRAFTING_SHAPELESS: Id<crate::RecipeDisplay> = Id::from_static(0);
pub const FURNACE: Id<crate::RecipeDisplay> = Id::from_static(2);
pub const SMITHING: Id<crate::RecipeDisplay> = Id::from_static(4);
pub const STONECUTTER: Id<crate::RecipeDisplay> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:crafting_shapeless",
    "minecraft:crafting_shaped",
    "minecraft:furnace",
    "minecraft:stonecutter",
    "minecraft:smithing",
];
