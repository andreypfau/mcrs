// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const COMPONENTS: Id<crate::EntitySubPredicateType> = Id::from_static(16);
pub const DISTANCE: Id<crate::EntitySubPredicateType> = Id::from_static(4);
pub const EFFECTS: Id<crate::EntitySubPredicateType> = Id::from_static(6);
pub const ENTITY_TAGS: Id<crate::EntitySubPredicateType> = Id::from_static(18);
pub const ENTITY_TYPE: Id<crate::EntitySubPredicateType> = Id::from_static(0);
pub const EQUIPMENT: Id<crate::EntitySubPredicateType> = Id::from_static(9);
pub const FLAGS: Id<crate::EntitySubPredicateType> = Id::from_static(8);
pub const LOCATION: Id<crate::EntitySubPredicateType> = Id::from_static(1);
pub const MOVEMENT: Id<crate::EntitySubPredicateType> = Id::from_static(5);
pub const MOVEMENT_AFFECTED_BY: Id<crate::EntitySubPredicateType> = Id::from_static(3);
pub const NBT: Id<crate::EntitySubPredicateType> = Id::from_static(7);
pub const PASSENGER: Id<crate::EntitySubPredicateType> = Id::from_static(12);
pub const PERIODIC_TICK: Id<crate::EntitySubPredicateType> = Id::from_static(10);
pub const PREDICATES: Id<crate::EntitySubPredicateType> = Id::from_static(17);
pub const SLOTS: Id<crate::EntitySubPredicateType> = Id::from_static(15);
pub const STEPPING_ON: Id<crate::EntitySubPredicateType> = Id::from_static(2);
pub const TARGETED_ENTITY: Id<crate::EntitySubPredicateType> = Id::from_static(13);
pub const TEAM: Id<crate::EntitySubPredicateType> = Id::from_static(14);
pub const TYPE_SPECIFIC_CUBE_MOB: Id<crate::EntitySubPredicateType> = Id::from_static(22);
pub const TYPE_SPECIFIC_FISHING_HOOK: Id<crate::EntitySubPredicateType> = Id::from_static(20);
pub const TYPE_SPECIFIC_LIGHTNING: Id<crate::EntitySubPredicateType> = Id::from_static(19);
pub const TYPE_SPECIFIC_PLAYER: Id<crate::EntitySubPredicateType> = Id::from_static(21);
pub const TYPE_SPECIFIC_RAIDER: Id<crate::EntitySubPredicateType> = Id::from_static(23);
pub const TYPE_SPECIFIC_SHEEP: Id<crate::EntitySubPredicateType> = Id::from_static(24);
pub const VEHICLE: Id<crate::EntitySubPredicateType> = Id::from_static(11);

pub const NAMES: &[&str] = &[
    "minecraft:entity_type",
    "minecraft:location",
    "minecraft:stepping_on",
    "minecraft:movement_affected_by",
    "minecraft:distance",
    "minecraft:movement",
    "minecraft:effects",
    "minecraft:nbt",
    "minecraft:flags",
    "minecraft:equipment",
    "minecraft:periodic_tick",
    "minecraft:vehicle",
    "minecraft:passenger",
    "minecraft:targeted_entity",
    "minecraft:team",
    "minecraft:slots",
    "minecraft:components",
    "minecraft:predicates",
    "minecraft:entity_tags",
    "minecraft:type_specific/lightning",
    "minecraft:type_specific/fishing_hook",
    "minecraft:type_specific/player",
    "minecraft:type_specific/cube_mob",
    "minecraft:type_specific/raider",
    "minecraft:type_specific/sheep",
];
