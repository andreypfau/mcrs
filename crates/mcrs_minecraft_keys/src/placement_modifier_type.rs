// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BIOME: Id<crate::PlacementModifierType> = Id::from_static(5);
pub const BLOCK_PREDICATE_FILTER: Id<crate::PlacementModifierType> = Id::from_static(0);
pub const COUNT: Id<crate::PlacementModifierType> = Id::from_static(6);
pub const COUNT_ON_EVERY_LAYER: Id<crate::PlacementModifierType> = Id::from_static(9);
pub const CUBOID: Id<crate::PlacementModifierType> = Id::from_static(10);
pub const ENVIRONMENT_SCAN: Id<crate::PlacementModifierType> = Id::from_static(11);
pub const FIXED_PLACEMENT: Id<crate::PlacementModifierType> = Id::from_static(17);
pub const HEIGHT_RANGE: Id<crate::PlacementModifierType> = Id::from_static(13);
pub const HEIGHTMAP: Id<crate::PlacementModifierType> = Id::from_static(12);
pub const IN_SQUARE: Id<crate::PlacementModifierType> = Id::from_static(14);
pub const NOISE_BASED_COUNT: Id<crate::PlacementModifierType> = Id::from_static(7);
pub const NOISE_THRESHOLD_COUNT: Id<crate::PlacementModifierType> = Id::from_static(8);
pub const OFFSET: Id<crate::PlacementModifierType> = Id::from_static(15);
pub const RANDOM_CHANCE: Id<crate::PlacementModifierType> = Id::from_static(2);
pub const RANDOMLY_SELECTED: Id<crate::PlacementModifierType> = Id::from_static(16);
pub const RARITY_FILTER: Id<crate::PlacementModifierType> = Id::from_static(1);
pub const SURFACE_RELATIVE_THRESHOLD_FILTER: Id<crate::PlacementModifierType> = Id::from_static(3);
pub const SURFACE_WATER_DEPTH_FILTER: Id<crate::PlacementModifierType> = Id::from_static(4);

pub const NAMES: &[&str] = &[
    "minecraft:block_predicate_filter",
    "minecraft:rarity_filter",
    "minecraft:random_chance",
    "minecraft:surface_relative_threshold_filter",
    "minecraft:surface_water_depth_filter",
    "minecraft:biome",
    "minecraft:count",
    "minecraft:noise_based_count",
    "minecraft:noise_threshold_count",
    "minecraft:count_on_every_layer",
    "minecraft:cuboid",
    "minecraft:environment_scan",
    "minecraft:heightmap",
    "minecraft:height_range",
    "minecraft:in_square",
    "minecraft:offset",
    "minecraft:randomly_selected",
    "minecraft:fixed_placement",
];
