// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLAST_FURNACE_BLOCKS: Id<crate::RecipeBookCategory> = Id::from_static(7);
pub const BLAST_FURNACE_MISC: Id<crate::RecipeBookCategory> = Id::from_static(8);
pub const CAMPFIRE: Id<crate::RecipeBookCategory> = Id::from_static(12);
pub const CRAFTING_BUILDING_BLOCKS: Id<crate::RecipeBookCategory> = Id::from_static(0);
pub const CRAFTING_EQUIPMENT: Id<crate::RecipeBookCategory> = Id::from_static(2);
pub const CRAFTING_MISC: Id<crate::RecipeBookCategory> = Id::from_static(3);
pub const CRAFTING_REDSTONE: Id<crate::RecipeBookCategory> = Id::from_static(1);
pub const FURNACE_BLOCKS: Id<crate::RecipeBookCategory> = Id::from_static(5);
pub const FURNACE_FOOD: Id<crate::RecipeBookCategory> = Id::from_static(4);
pub const FURNACE_MISC: Id<crate::RecipeBookCategory> = Id::from_static(6);
pub const SMITHING: Id<crate::RecipeBookCategory> = Id::from_static(11);
pub const SMOKER_FOOD: Id<crate::RecipeBookCategory> = Id::from_static(9);
pub const STONECUTTER: Id<crate::RecipeBookCategory> = Id::from_static(10);

pub const NAMES: &[&str] = &[
    "minecraft:crafting_building_blocks",
    "minecraft:crafting_redstone",
    "minecraft:crafting_equipment",
    "minecraft:crafting_misc",
    "minecraft:furnace_food",
    "minecraft:furnace_blocks",
    "minecraft:furnace_misc",
    "minecraft:blast_furnace_blocks",
    "minecraft:blast_furnace_misc",
    "minecraft:smoker_food",
    "minecraft:stonecutter",
    "minecraft:smithing",
    "minecraft:campfire",
];
