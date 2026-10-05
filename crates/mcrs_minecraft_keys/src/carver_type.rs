// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CANYON: Id<crate::CarverType> = Id::from_static(1);
pub const CAVE: Id<crate::CarverType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:cave",
    "minecraft:canyon",
];
