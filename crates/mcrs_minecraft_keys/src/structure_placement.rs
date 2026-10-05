// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CONCENTRIC_RINGS: Id<crate::StructurePlacement> = Id::from_static(0);
pub const DIMENSION_ORIGIN: Id<crate::StructurePlacement> = Id::from_static(1);
pub const RANDOM_SPREAD: Id<crate::StructurePlacement> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:concentric_rings",
    "minecraft:dimension_origin",
    "minecraft:random_spread",
];
