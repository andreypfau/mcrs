// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ATOM: Id<crate::PermissionType> = Id::from_static(0);
pub const COMMAND_LEVEL: Id<crate::PermissionType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:atom",
    "minecraft:command_level",
];
