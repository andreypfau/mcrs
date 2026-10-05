// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLANK: Id<crate::NumberFormatType> = Id::from_static(0);
pub const FIXED: Id<crate::NumberFormatType> = Id::from_static(2);
pub const STYLED: Id<crate::NumberFormatType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:blank",
    "minecraft:styled",
    "minecraft:fixed",
];
