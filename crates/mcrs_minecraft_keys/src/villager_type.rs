// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const DESERT: Id<crate::VillagerType> = Id::from_static(0);
pub const JUNGLE: Id<crate::VillagerType> = Id::from_static(1);
pub const PLAINS: Id<crate::VillagerType> = Id::from_static(2);
pub const SAVANNA: Id<crate::VillagerType> = Id::from_static(3);
pub const SNOW: Id<crate::VillagerType> = Id::from_static(4);
pub const SWAMP: Id<crate::VillagerType> = Id::from_static(5);
pub const TAIGA: Id<crate::VillagerType> = Id::from_static(6);

pub const NAMES: &[&str] = &[
    "minecraft:desert",
    "minecraft:jungle",
    "minecraft:plains",
    "minecraft:savanna",
    "minecraft:snow",
    "minecraft:swamp",
    "minecraft:taiga",
];
