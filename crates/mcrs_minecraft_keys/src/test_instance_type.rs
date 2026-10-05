// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLOCK_BASED: Id<crate::TestInstanceType> = Id::from_static(0);
pub const FUNCTION: Id<crate::TestInstanceType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:block_based",
    "minecraft:function",
];
