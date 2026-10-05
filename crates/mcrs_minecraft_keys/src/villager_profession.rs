// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ARMORER: Id<crate::VillagerProfession> = Id::from_static(1);
pub const BUTCHER: Id<crate::VillagerProfession> = Id::from_static(2);
pub const CARTOGRAPHER: Id<crate::VillagerProfession> = Id::from_static(3);
pub const CLERIC: Id<crate::VillagerProfession> = Id::from_static(4);
pub const FARMER: Id<crate::VillagerProfession> = Id::from_static(5);
pub const FISHERMAN: Id<crate::VillagerProfession> = Id::from_static(6);
pub const FLETCHER: Id<crate::VillagerProfession> = Id::from_static(7);
pub const LEATHERWORKER: Id<crate::VillagerProfession> = Id::from_static(8);
pub const LIBRARIAN: Id<crate::VillagerProfession> = Id::from_static(9);
pub const MASON: Id<crate::VillagerProfession> = Id::from_static(10);
pub const NITWIT: Id<crate::VillagerProfession> = Id::from_static(11);
pub const NONE: Id<crate::VillagerProfession> = Id::from_static(0);
pub const SHEPHERD: Id<crate::VillagerProfession> = Id::from_static(12);
pub const TOOLSMITH: Id<crate::VillagerProfession> = Id::from_static(13);
pub const WEAPONSMITH: Id<crate::VillagerProfession> = Id::from_static(14);

pub const NAMES: &[&str] = &[
    "minecraft:none",
    "minecraft:armorer",
    "minecraft:butcher",
    "minecraft:cartographer",
    "minecraft:cleric",
    "minecraft:farmer",
    "minecraft:fisherman",
    "minecraft:fletcher",
    "minecraft:leatherworker",
    "minecraft:librarian",
    "minecraft:mason",
    "minecraft:nitwit",
    "minecraft:shepherd",
    "minecraft:toolsmith",
    "minecraft:weaponsmith",
];
