// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ANY_FUEL: Id<crate::SlotDisplay> = Id::from_static(1);
pub const COMPOSITE: Id<crate::SlotDisplay> = Id::from_static(10);
pub const DYED: Id<crate::SlotDisplay> = Id::from_static(7);
pub const EMPTY: Id<crate::SlotDisplay> = Id::from_static(0);
pub const ITEM: Id<crate::SlotDisplay> = Id::from_static(4);
pub const ITEM_STACK: Id<crate::SlotDisplay> = Id::from_static(5);
pub const ONLY_WITH_COMPONENT: Id<crate::SlotDisplay> = Id::from_static(3);
pub const SMITHING_TRIM: Id<crate::SlotDisplay> = Id::from_static(8);
pub const TAG: Id<crate::SlotDisplay> = Id::from_static(6);
pub const WITH_ANY_POTION: Id<crate::SlotDisplay> = Id::from_static(2);
pub const WITH_REMAINDER: Id<crate::SlotDisplay> = Id::from_static(9);

pub const NAMES: &[&str] = &[
    "minecraft:empty",
    "minecraft:any_fuel",
    "minecraft:with_any_potion",
    "minecraft:only_with_component",
    "minecraft:item",
    "minecraft:item_stack",
    "minecraft:tag",
    "minecraft:dyed",
    "minecraft:smithing_trim",
    "minecraft:with_remainder",
    "minecraft:composite",
];
