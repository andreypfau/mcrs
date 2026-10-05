// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALTERNATIVES: Id<crate::LootPoolEntryType> = Id::from_static(6);
pub const DYNAMIC: Id<crate::LootPoolEntryType> = Id::from_static(3);
pub const EMPTY: Id<crate::LootPoolEntryType> = Id::from_static(0);
pub const GROUP: Id<crate::LootPoolEntryType> = Id::from_static(8);
pub const ITEM: Id<crate::LootPoolEntryType> = Id::from_static(1);
pub const LOOT_TABLE: Id<crate::LootPoolEntryType> = Id::from_static(2);
pub const SEQUENCE: Id<crate::LootPoolEntryType> = Id::from_static(7);
pub const SLOTS: Id<crate::LootPoolEntryType> = Id::from_static(5);
pub const TAG: Id<crate::LootPoolEntryType> = Id::from_static(4);

pub const NAMES: &[&str] = &[
    "minecraft:empty",
    "minecraft:item",
    "minecraft:loot_table",
    "minecraft:dynamic",
    "minecraft:tag",
    "minecraft:slots",
    "minecraft:alternatives",
    "minecraft:sequence",
    "minecraft:group",
];
