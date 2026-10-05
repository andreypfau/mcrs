// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ABOVE_PRELIMINARY_SURFACE: Id<crate::MaterialConditionType> = Id::from_static(9);
pub const BIOME: Id<crate::MaterialConditionType> = Id::from_static(0);
pub const HOLE: Id<crate::MaterialConditionType> = Id::from_static(8);
pub const NOISE_THRESHOLD: Id<crate::MaterialConditionType> = Id::from_static(1);
pub const NOT: Id<crate::MaterialConditionType> = Id::from_static(7);
pub const STEEP: Id<crate::MaterialConditionType> = Id::from_static(6);
pub const STONE_DEPTH: Id<crate::MaterialConditionType> = Id::from_static(10);
pub const TEMPERATURE: Id<crate::MaterialConditionType> = Id::from_static(5);
pub const VERTICAL_GRADIENT: Id<crate::MaterialConditionType> = Id::from_static(2);
pub const WATER: Id<crate::MaterialConditionType> = Id::from_static(4);
pub const Y_ABOVE: Id<crate::MaterialConditionType> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:biome",
    "minecraft:noise_threshold",
    "minecraft:vertical_gradient",
    "minecraft:y_above",
    "minecraft:water",
    "minecraft:temperature",
    "minecraft:steep",
    "minecraft:not",
    "minecraft:hole",
    "minecraft:above_preliminary_surface",
    "minecraft:stone_depth",
];
