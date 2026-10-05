// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BOOLEAN: Id<crate::InputControlType> = Id::from_static(0);
pub const NUMBER_RANGE: Id<crate::InputControlType> = Id::from_static(1);
pub const SINGLE_OPTION: Id<crate::InputControlType> = Id::from_static(2);
pub const TEXT: Id<crate::InputControlType> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:boolean",
    "minecraft:number_range",
    "minecraft:single_option",
    "minecraft:text",
];
