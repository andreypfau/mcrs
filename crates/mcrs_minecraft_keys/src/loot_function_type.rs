// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const APPLY_BONUS: Id<crate::LootFunctionType> = Id::from_static(19);
pub const COPY_COMPONENTS: Id<crate::LootFunctionType> = Id::from_static(32);
pub const COPY_CUSTOM_DATA: Id<crate::LootFunctionType> = Id::from_static(24);
pub const COPY_NAME: Id<crate::LootFunctionType> = Id::from_static(14);
pub const COPY_STATE: Id<crate::LootFunctionType> = Id::from_static(25);
pub const DISCARD: Id<crate::LootFunctionType> = Id::from_static(41);
pub const ENCHANT_RANDOMLY: Id<crate::LootFunctionType> = Id::from_static(3);
pub const ENCHANT_WITH_LEVELS: Id<crate::LootFunctionType> = Id::from_static(2);
pub const ENCHANTED_COUNT_INCREASE: Id<crate::LootFunctionType> = Id::from_static(8);
pub const EXPLORATION_MAP: Id<crate::LootFunctionType> = Id::from_static(12);
pub const EXPLOSION_DECAY: Id<crate::LootFunctionType> = Id::from_static(21);
pub const FILL_PLAYER_HEAD: Id<crate::LootFunctionType> = Id::from_static(23);
pub const FILTERED: Id<crate::LootFunctionType> = Id::from_static(17);
pub const FURNACE_SMELT: Id<crate::LootFunctionType> = Id::from_static(7);
pub const LIMIT_COUNT: Id<crate::LootFunctionType> = Id::from_static(18);
pub const MODIFY_CONTENTS: Id<crate::LootFunctionType> = Id::from_static(16);
pub const SEQUENCE: Id<crate::LootFunctionType> = Id::from_static(31);
pub const SET_ATTRIBUTES: Id<crate::LootFunctionType> = Id::from_static(10);
pub const SET_BANNER_PATTERN: Id<crate::LootFunctionType> = Id::from_static(26);
pub const SET_BOOK_COVER: Id<crate::LootFunctionType> = Id::from_static(35);
pub const SET_COMPONENTS: Id<crate::LootFunctionType> = Id::from_static(6);
pub const SET_CONTENTS: Id<crate::LootFunctionType> = Id::from_static(15);
pub const SET_COUNT: Id<crate::LootFunctionType> = Id::from_static(0);
pub const SET_CUSTOM_DATA: Id<crate::LootFunctionType> = Id::from_static(5);
pub const SET_CUSTOM_MODEL_DATA: Id<crate::LootFunctionType> = Id::from_static(40);
pub const SET_DAMAGE: Id<crate::LootFunctionType> = Id::from_static(9);
pub const SET_ENCHANTMENTS: Id<crate::LootFunctionType> = Id::from_static(4);
pub const SET_FIREWORK_EXPLOSION: Id<crate::LootFunctionType> = Id::from_static(34);
pub const SET_FIREWORKS: Id<crate::LootFunctionType> = Id::from_static(33);
pub const SET_INSTRUMENT: Id<crate::LootFunctionType> = Id::from_static(30);
pub const SET_ITEM: Id<crate::LootFunctionType> = Id::from_static(1);
pub const SET_LOOT_TABLE: Id<crate::LootFunctionType> = Id::from_static(20);
pub const SET_LORE: Id<crate::LootFunctionType> = Id::from_static(22);
pub const SET_NAME: Id<crate::LootFunctionType> = Id::from_static(11);
pub const SET_OMINOUS_BOTTLE_AMPLIFIER: Id<crate::LootFunctionType> = Id::from_static(39);
pub const SET_POTION: Id<crate::LootFunctionType> = Id::from_static(27);
pub const SET_RANDOM_DYES: Id<crate::LootFunctionType> = Id::from_static(28);
pub const SET_RANDOM_POTION: Id<crate::LootFunctionType> = Id::from_static(29);
pub const SET_STEW_EFFECT: Id<crate::LootFunctionType> = Id::from_static(13);
pub const SET_WRITABLE_BOOK_PAGES: Id<crate::LootFunctionType> = Id::from_static(37);
pub const SET_WRITTEN_BOOK_PAGES: Id<crate::LootFunctionType> = Id::from_static(36);
pub const TOGGLE_TOOLTIPS: Id<crate::LootFunctionType> = Id::from_static(38);

pub const NAMES: &[&str] = &[
    "minecraft:set_count",
    "minecraft:set_item",
    "minecraft:enchant_with_levels",
    "minecraft:enchant_randomly",
    "minecraft:set_enchantments",
    "minecraft:set_custom_data",
    "minecraft:set_components",
    "minecraft:furnace_smelt",
    "minecraft:enchanted_count_increase",
    "minecraft:set_damage",
    "minecraft:set_attributes",
    "minecraft:set_name",
    "minecraft:exploration_map",
    "minecraft:set_stew_effect",
    "minecraft:copy_name",
    "minecraft:set_contents",
    "minecraft:modify_contents",
    "minecraft:filtered",
    "minecraft:limit_count",
    "minecraft:apply_bonus",
    "minecraft:set_loot_table",
    "minecraft:explosion_decay",
    "minecraft:set_lore",
    "minecraft:fill_player_head",
    "minecraft:copy_custom_data",
    "minecraft:copy_state",
    "minecraft:set_banner_pattern",
    "minecraft:set_potion",
    "minecraft:set_random_dyes",
    "minecraft:set_random_potion",
    "minecraft:set_instrument",
    "minecraft:sequence",
    "minecraft:copy_components",
    "minecraft:set_fireworks",
    "minecraft:set_firework_explosion",
    "minecraft:set_book_cover",
    "minecraft:set_written_book_pages",
    "minecraft:set_writable_book_pages",
    "minecraft:toggle_tooltips",
    "minecraft:set_ominous_bottle_amplifier",
    "minecraft:set_custom_model_data",
    "minecraft:discard",
];
