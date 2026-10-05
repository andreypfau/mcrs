// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BURIED_TREASURE: Id<crate::StructureType> = Id::from_static(0);
pub const DESERT_PYRAMID: Id<crate::StructureType> = Id::from_static(1);
pub const END_CITY: Id<crate::StructureType> = Id::from_static(2);
pub const FORTRESS: Id<crate::StructureType> = Id::from_static(3);
pub const IGLOO: Id<crate::StructureType> = Id::from_static(4);
pub const JIGSAW: Id<crate::StructureType> = Id::from_static(5);
pub const JUNGLE_TEMPLE: Id<crate::StructureType> = Id::from_static(6);
pub const MINESHAFT: Id<crate::StructureType> = Id::from_static(7);
pub const NETHER_FOSSIL: Id<crate::StructureType> = Id::from_static(8);
pub const OCEAN_MONUMENT: Id<crate::StructureType> = Id::from_static(9);
pub const OCEAN_RUIN: Id<crate::StructureType> = Id::from_static(10);
pub const RUINED_PORTAL: Id<crate::StructureType> = Id::from_static(11);
pub const SHIPWRECK: Id<crate::StructureType> = Id::from_static(12);
pub const STRONGHOLD: Id<crate::StructureType> = Id::from_static(13);
pub const SWAMP_HUT: Id<crate::StructureType> = Id::from_static(14);
pub const WOODLAND_MANSION: Id<crate::StructureType> = Id::from_static(15);

pub const NAMES: &[&str] = &[
    "minecraft:buried_treasure",
    "minecraft:desert_pyramid",
    "minecraft:end_city",
    "minecraft:fortress",
    "minecraft:igloo",
    "minecraft:jigsaw",
    "minecraft:jungle_temple",
    "minecraft:mineshaft",
    "minecraft:nether_fossil",
    "minecraft:ocean_monument",
    "minecraft:ocean_ruin",
    "minecraft:ruined_portal",
    "minecraft:shipwreck",
    "minecraft:stronghold",
    "minecraft:swamp_hut",
    "minecraft:woodland_mansion",
];
