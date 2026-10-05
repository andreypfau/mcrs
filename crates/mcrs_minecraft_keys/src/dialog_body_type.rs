// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ITEM: Id<crate::DialogBodyType> = Id::from_static(0);
pub const PLAIN_MESSAGE: Id<crate::DialogBodyType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:item",
    "minecraft:plain_message",
];
