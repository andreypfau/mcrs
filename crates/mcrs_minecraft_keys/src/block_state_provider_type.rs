// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const COPY_PROPERTIES: Id<crate::BlockStateProviderType> = Id::from_static(0);
pub const DUAL_NOISE: Id<crate::BlockStateProviderType> = Id::from_static(1);
pub const NOISE: Id<crate::BlockStateProviderType> = Id::from_static(2);
pub const NOISE_THRESHOLD: Id<crate::BlockStateProviderType> = Id::from_static(3);
pub const RANDOM_BLOCK: Id<crate::BlockStateProviderType> = Id::from_static(4);
pub const RANDOMIZED_INT: Id<crate::BlockStateProviderType> = Id::from_static(5);
pub const ROTATED: Id<crate::BlockStateProviderType> = Id::from_static(6);
pub const RULE_BASED: Id<crate::BlockStateProviderType> = Id::from_static(7);
pub const SIMPLE: Id<crate::BlockStateProviderType> = Id::from_static(8);
pub const WEIGHTED: Id<crate::BlockStateProviderType> = Id::from_static(9);

pub const NAMES: &[&str] = &[
    "minecraft:copy_properties",
    "minecraft:dual_noise",
    "minecraft:noise",
    "minecraft:noise_threshold",
    "minecraft:random_block",
    "minecraft:randomized_int",
    "minecraft:rotated",
    "minecraft:rule_based",
    "minecraft:simple",
    "minecraft:weighted",
];
