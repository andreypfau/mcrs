// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const DIRECT: Id<crate::PoolAliasBinding> = Id::from_static(2);
pub const RANDOM: Id<crate::PoolAliasBinding> = Id::from_static(0);
pub const RANDOM_GROUP: Id<crate::PoolAliasBinding> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:random",
    "minecraft:random_group",
    "minecraft:direct",
];
