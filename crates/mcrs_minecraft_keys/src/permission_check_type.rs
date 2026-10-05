// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALWAYS_PASS: Id<crate::PermissionCheckType> = Id::from_static(0);
pub const REQUIRE: Id<crate::PermissionCheckType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:always_pass",
    "minecraft:require",
];
