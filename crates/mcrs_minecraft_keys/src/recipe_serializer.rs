// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLASTING: Id<crate::RecipeSerializer> = Id::from_static(15);
pub const BREWING: Id<crate::RecipeSerializer> = Id::from_static(20);
pub const CAMPFIRE_COOKING: Id<crate::RecipeSerializer> = Id::from_static(17);
pub const CRAFTING_DECORATED_POT: Id<crate::RecipeSerializer> = Id::from_static(5);
pub const CRAFTING_DYE: Id<crate::RecipeSerializer> = Id::from_static(2);
pub const CRAFTING_IMBUE: Id<crate::RecipeSerializer> = Id::from_static(3);
pub const CRAFTING_SHAPED: Id<crate::RecipeSerializer> = Id::from_static(0);
pub const CRAFTING_SHAPELESS: Id<crate::RecipeSerializer> = Id::from_static(1);
pub const CRAFTING_SPECIAL_BANNERDUPLICATE: Id<crate::RecipeSerializer> = Id::from_static(11);
pub const CRAFTING_SPECIAL_BOOKCLONING: Id<crate::RecipeSerializer> = Id::from_static(6);
pub const CRAFTING_SPECIAL_FIREWORK_ROCKET: Id<crate::RecipeSerializer> = Id::from_static(8);
pub const CRAFTING_SPECIAL_FIREWORK_STAR: Id<crate::RecipeSerializer> = Id::from_static(9);
pub const CRAFTING_SPECIAL_FIREWORK_STAR_FADE: Id<crate::RecipeSerializer> = Id::from_static(10);
pub const CRAFTING_SPECIAL_MAPEXTENDING: Id<crate::RecipeSerializer> = Id::from_static(7);
pub const CRAFTING_SPECIAL_REPAIRITEM: Id<crate::RecipeSerializer> = Id::from_static(13);
pub const CRAFTING_SPECIAL_SHIELDDECORATION: Id<crate::RecipeSerializer> = Id::from_static(12);
pub const CRAFTING_TRANSMUTE: Id<crate::RecipeSerializer> = Id::from_static(4);
pub const SMELTING: Id<crate::RecipeSerializer> = Id::from_static(14);
pub const SMITHING_TRANSFORM: Id<crate::RecipeSerializer> = Id::from_static(19);
pub const SMITHING_TRIM: Id<crate::RecipeSerializer> = Id::from_static(21);
pub const SMOKING: Id<crate::RecipeSerializer> = Id::from_static(16);
pub const STONECUTTING: Id<crate::RecipeSerializer> = Id::from_static(18);

pub const NAMES: &[&str] = &[
    "minecraft:crafting_shaped",
    "minecraft:crafting_shapeless",
    "minecraft:crafting_dye",
    "minecraft:crafting_imbue",
    "minecraft:crafting_transmute",
    "minecraft:crafting_decorated_pot",
    "minecraft:crafting_special_bookcloning",
    "minecraft:crafting_special_mapextending",
    "minecraft:crafting_special_firework_rocket",
    "minecraft:crafting_special_firework_star",
    "minecraft:crafting_special_firework_star_fade",
    "minecraft:crafting_special_bannerduplicate",
    "minecraft:crafting_special_shielddecoration",
    "minecraft:crafting_special_repairitem",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing_transform",
    "minecraft:brewing",
    "minecraft:smithing_trim",
];
