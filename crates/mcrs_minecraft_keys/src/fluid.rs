// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const EMPTY: Id<crate::Fluid> = Id::from_static(0);
pub const FLOWING_LAVA: Id<crate::Fluid> = Id::from_static(3);
pub const FLOWING_WATER: Id<crate::Fluid> = Id::from_static(1);
pub const LAVA: Id<crate::Fluid> = Id::from_static(4);
pub const WATER: Id<crate::Fluid> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:empty",
    "minecraft:flowing_water",
    "minecraft:water",
    "minecraft:flowing_lava",
    "minecraft:lava",
];
