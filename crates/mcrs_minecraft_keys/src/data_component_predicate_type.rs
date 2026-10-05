// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ATTRIBUTE_MODIFIERS: Id<crate::DataComponentPredicateType> = Id::from_static(11);
pub const BUNDLE_CONTENTS: Id<crate::DataComponentPredicateType> = Id::from_static(6);
pub const CONTAINER: Id<crate::DataComponentPredicateType> = Id::from_static(5);
pub const CUSTOM_DATA: Id<crate::DataComponentPredicateType> = Id::from_static(4);
pub const DAMAGE: Id<crate::DataComponentPredicateType> = Id::from_static(0);
pub const ENCHANTMENTS: Id<crate::DataComponentPredicateType> = Id::from_static(1);
pub const FIREWORK_EXPLOSION: Id<crate::DataComponentPredicateType> = Id::from_static(7);
pub const FIREWORKS: Id<crate::DataComponentPredicateType> = Id::from_static(8);
pub const JUKEBOX_PLAYABLE: Id<crate::DataComponentPredicateType> = Id::from_static(13);
pub const POTION_CONTENTS: Id<crate::DataComponentPredicateType> = Id::from_static(3);
pub const STORED_ENCHANTMENTS: Id<crate::DataComponentPredicateType> = Id::from_static(2);
pub const TRIM: Id<crate::DataComponentPredicateType> = Id::from_static(12);
pub const VILLAGER_VARIANT: Id<crate::DataComponentPredicateType> = Id::from_static(14);
pub const WRITABLE_BOOK_CONTENT: Id<crate::DataComponentPredicateType> = Id::from_static(9);
pub const WRITTEN_BOOK_CONTENT: Id<crate::DataComponentPredicateType> = Id::from_static(10);

pub const NAMES: &[&str] = &[
    "minecraft:damage",
    "minecraft:enchantments",
    "minecraft:stored_enchantments",
    "minecraft:potion_contents",
    "minecraft:custom_data",
    "minecraft:container",
    "minecraft:bundle_contents",
    "minecraft:firework_explosion",
    "minecraft:fireworks",
    "minecraft:writable_book_content",
    "minecraft:written_book_content",
    "minecraft:attribute_modifiers",
    "minecraft:trim",
    "minecraft:jukebox_playable",
    "minecraft:villager/variant",
];
