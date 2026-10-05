// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ABS: Id<crate::DensityFunctionType> = Id::from_static(11);
pub const ADD: Id<crate::DensityFunctionType> = Id::from_static(26);
pub const BEARDIFIER: Id<crate::DensityFunctionType> = Id::from_static(3);
pub const BLEND_ALPHA: Id<crate::DensityFunctionType> = Id::from_static(1);
pub const BLEND_DENSITY: Id<crate::DensityFunctionType> = Id::from_static(39);
pub const BLEND_OFFSET: Id<crate::DensityFunctionType> = Id::from_static(2);
pub const CACHE: Id<crate::DensityFunctionType> = Id::from_static(38);
pub const CEIL: Id<crate::DensityFunctionType> = Id::from_static(24);
pub const CLAMP: Id<crate::DensityFunctionType> = Id::from_static(35);
pub const CONSTANT: Id<crate::DensityFunctionType> = Id::from_static(0);
pub const CUBE: Id<crate::DensityFunctionType> = Id::from_static(13);
pub const DISTANCE_TO_POINT: Id<crate::DensityFunctionType> = Id::from_static(6);
pub const DIV: Id<crate::DensityFunctionType> = Id::from_static(29);
pub const END_OUTER_ISLANDS: Id<crate::DensityFunctionType> = Id::from_static(5);
pub const FIND_TOP_SURFACE: Id<crate::DensityFunctionType> = Id::from_static(42);
pub const FLOOR: Id<crate::DensityFunctionType> = Id::from_static(22);
pub const GRADIENT: Id<crate::DensityFunctionType> = Id::from_static(7);
pub const HALF_NEGATIVE: Id<crate::DensityFunctionType> = Id::from_static(15);
pub const INTERPOLATED: Id<crate::DensityFunctionType> = Id::from_static(40);
pub const INTERVAL_SELECT: Id<crate::DensityFunctionType> = Id::from_static(37);
pub const LERP: Id<crate::DensityFunctionType> = Id::from_static(34);
pub const LOG: Id<crate::DensityFunctionType> = Id::from_static(20);
pub const MAX: Id<crate::DensityFunctionType> = Id::from_static(31);
pub const MIN: Id<crate::DensityFunctionType> = Id::from_static(30);
pub const MUL: Id<crate::DensityFunctionType> = Id::from_static(28);
pub const NEGATE: Id<crate::DensityFunctionType> = Id::from_static(18);
pub const NOISE: Id<crate::DensityFunctionType> = Id::from_static(4);
pub const OLD_BLENDED_NOISE: Id<crate::DensityFunctionType> = Id::from_static(43);
pub const POW: Id<crate::DensityFunctionType> = Id::from_static(32);
pub const QUARTER_NEGATIVE: Id<crate::DensityFunctionType> = Id::from_static(16);
pub const RANGE_CHOICE: Id<crate::DensityFunctionType> = Id::from_static(36);
pub const RECIPROCAL: Id<crate::DensityFunctionType> = Id::from_static(17);
pub const ROUND: Id<crate::DensityFunctionType> = Id::from_static(23);
pub const SHIFT: Id<crate::DensityFunctionType> = Id::from_static(10);
pub const SHIFT_A: Id<crate::DensityFunctionType> = Id::from_static(8);
pub const SHIFT_B: Id<crate::DensityFunctionType> = Id::from_static(9);
pub const SIGN: Id<crate::DensityFunctionType> = Id::from_static(21);
pub const SLICE: Id<crate::DensityFunctionType> = Id::from_static(41);
pub const SPLINE: Id<crate::DensityFunctionType> = Id::from_static(33);
pub const SQRT: Id<crate::DensityFunctionType> = Id::from_static(14);
pub const SQUARE: Id<crate::DensityFunctionType> = Id::from_static(12);
pub const SQUEEZE: Id<crate::DensityFunctionType> = Id::from_static(19);
pub const SUB: Id<crate::DensityFunctionType> = Id::from_static(27);
pub const TRUNCATE: Id<crate::DensityFunctionType> = Id::from_static(25);

pub const NAMES: &[&str] = &[
    "minecraft:constant",
    "minecraft:blend_alpha",
    "minecraft:blend_offset",
    "minecraft:beardifier",
    "minecraft:noise",
    "minecraft:end_outer_islands",
    "minecraft:distance_to_point",
    "minecraft:gradient",
    "minecraft:shift_a",
    "minecraft:shift_b",
    "minecraft:shift",
    "minecraft:abs",
    "minecraft:square",
    "minecraft:cube",
    "minecraft:sqrt",
    "minecraft:half_negative",
    "minecraft:quarter_negative",
    "minecraft:reciprocal",
    "minecraft:negate",
    "minecraft:squeeze",
    "minecraft:log",
    "minecraft:sign",
    "minecraft:floor",
    "minecraft:round",
    "minecraft:ceil",
    "minecraft:truncate",
    "minecraft:add",
    "minecraft:sub",
    "minecraft:mul",
    "minecraft:div",
    "minecraft:min",
    "minecraft:max",
    "minecraft:pow",
    "minecraft:spline",
    "minecraft:lerp",
    "minecraft:clamp",
    "minecraft:range_choice",
    "minecraft:interval_select",
    "minecraft:cache",
    "minecraft:blend_density",
    "minecraft:interpolated",
    "minecraft:slice",
    "minecraft:find_top_surface",
    "minecraft:old_blended_noise",
];
