// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BENDING_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(6);
pub const CHERRY_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(8);
pub const DARK_OAK_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(4);
pub const FANCY_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(5);
pub const FORKING_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(1);
pub const GIANT_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(2);
pub const MEGA_JUNGLE_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(3);
pub const POPLAR_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(9);
pub const STRAIGHT_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(0);
pub const UPWARDS_BRANCHING_TRUNK_PLACER: Id<crate::TrunkPlacerType> = Id::from_static(7);

pub const NAMES: &[&str] = &[
    "minecraft:straight_trunk_placer",
    "minecraft:forking_trunk_placer",
    "minecraft:giant_trunk_placer",
    "minecraft:mega_jungle_trunk_placer",
    "minecraft:dark_oak_trunk_placer",
    "minecraft:fancy_trunk_placer",
    "minecraft:bending_trunk_placer",
    "minecraft:upwards_branching_trunk_placer",
    "minecraft:cherry_trunk_placer",
    "minecraft:poplar_trunk_placer",
];
