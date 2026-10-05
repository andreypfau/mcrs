// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ARMORER: Id<crate::PointOfInterestType> = Id::from_static(0);
pub const BEE_NEST: Id<crate::PointOfInterestType> = Id::from_static(16);
pub const BEEHIVE: Id<crate::PointOfInterestType> = Id::from_static(15);
pub const BUTCHER: Id<crate::PointOfInterestType> = Id::from_static(1);
pub const CARTOGRAPHER: Id<crate::PointOfInterestType> = Id::from_static(2);
pub const CLERIC: Id<crate::PointOfInterestType> = Id::from_static(3);
pub const FARMER: Id<crate::PointOfInterestType> = Id::from_static(4);
pub const FISHERMAN: Id<crate::PointOfInterestType> = Id::from_static(5);
pub const FLETCHER: Id<crate::PointOfInterestType> = Id::from_static(6);
pub const HOME: Id<crate::PointOfInterestType> = Id::from_static(13);
pub const LEATHERWORKER: Id<crate::PointOfInterestType> = Id::from_static(7);
pub const LIBRARIAN: Id<crate::PointOfInterestType> = Id::from_static(8);
pub const LIGHTNING_ROD: Id<crate::PointOfInterestType> = Id::from_static(20);
pub const LODESTONE: Id<crate::PointOfInterestType> = Id::from_static(18);
pub const MASON: Id<crate::PointOfInterestType> = Id::from_static(9);
pub const MEETING: Id<crate::PointOfInterestType> = Id::from_static(14);
pub const NETHER_PORTAL: Id<crate::PointOfInterestType> = Id::from_static(17);
pub const SHEPHERD: Id<crate::PointOfInterestType> = Id::from_static(10);
pub const TEST_INSTANCE: Id<crate::PointOfInterestType> = Id::from_static(19);
pub const TOOLSMITH: Id<crate::PointOfInterestType> = Id::from_static(11);
pub const WEAPONSMITH: Id<crate::PointOfInterestType> = Id::from_static(12);

pub const NAMES: &[&str] = &[
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
    "minecraft:shepherd",
    "minecraft:toolsmith",
    "minecraft:weaponsmith",
    "minecraft:home",
    "minecraft:meeting",
    "minecraft:beehive",
    "minecraft:bee_nest",
    "minecraft:nether_portal",
    "minecraft:lodestone",
    "minecraft:test_instance",
    "minecraft:lightning_rod",
];
