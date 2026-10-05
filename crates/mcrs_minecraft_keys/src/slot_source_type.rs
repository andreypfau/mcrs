// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CONTENTS: Id<crate::SlotSourceType> = Id::from_static(4);
pub const EMPTY: Id<crate::SlotSourceType> = Id::from_static(5);
pub const FILTERED: Id<crate::SlotSourceType> = Id::from_static(1);
pub const GROUP: Id<crate::SlotSourceType> = Id::from_static(0);
pub const LIMIT_SLOTS: Id<crate::SlotSourceType> = Id::from_static(2);
pub const SLOT_RANGE: Id<crate::SlotSourceType> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:group",
    "minecraft:filtered",
    "minecraft:limit_slots",
    "minecraft:slot_range",
    "minecraft:contents",
    "minecraft:empty",
];
