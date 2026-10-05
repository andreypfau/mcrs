// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BROKEN: Id<crate::StatType> = Id::from_static(3);
pub const CRAFTED: Id<crate::StatType> = Id::from_static(1);
pub const CUSTOM: Id<crate::StatType> = Id::from_static(8);
pub const DROPPED: Id<crate::StatType> = Id::from_static(5);
pub const KILLED: Id<crate::StatType> = Id::from_static(6);
pub const KILLED_BY: Id<crate::StatType> = Id::from_static(7);
pub const MINED: Id<crate::StatType> = Id::from_static(0);
pub const PICKED_UP: Id<crate::StatType> = Id::from_static(4);
pub const USED: Id<crate::StatType> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:mined",
    "minecraft:crafted",
    "minecraft:used",
    "minecraft:broken",
    "minecraft:picked_up",
    "minecraft:dropped",
    "minecraft:killed",
    "minecraft:killed_by",
    "minecraft:custom",
];
