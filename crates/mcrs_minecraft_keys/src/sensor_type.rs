// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ARMADILLO_SCARE_DETECTED: Id<crate::SensorType> = Id::from_static(10);
pub const AXOLOTL_ATTACKABLES: Id<crate::SensorType> = Id::from_static(16);
pub const BREEZE_ATTACK_ENTITY_SENSOR: Id<crate::SensorType> = Id::from_static(23);
pub const DUMMY: Id<crate::SensorType> = Id::from_static(0);
pub const FOOD_TEMPTATIONS: Id<crate::SensorType> = Id::from_static(17);
pub const FROG_ATTACKABLES: Id<crate::SensorType> = Id::from_static(20);
pub const FROG_TEMPTATIONS: Id<crate::SensorType> = Id::from_static(18);
pub const GOLEM_DETECTED: Id<crate::SensorType> = Id::from_static(9);
pub const HOGLIN_SPECIFIC_SENSOR: Id<crate::SensorType> = Id::from_static(13);
pub const HURT_BY: Id<crate::SensorType> = Id::from_static(5);
pub const IS_IN_WATER: Id<crate::SensorType> = Id::from_static(21);
pub const NAUTILUS_TEMPTATIONS: Id<crate::SensorType> = Id::from_static(19);
pub const NEAREST_ADULT: Id<crate::SensorType> = Id::from_static(14);
pub const NEAREST_ADULT_ANY_TYPE: Id<crate::SensorType> = Id::from_static(15);
pub const NEAREST_BED: Id<crate::SensorType> = Id::from_static(4);
pub const NEAREST_ITEMS: Id<crate::SensorType> = Id::from_static(1);
pub const NEAREST_LIVING_ENTITIES: Id<crate::SensorType> = Id::from_static(2);
pub const NEAREST_PLAYERS: Id<crate::SensorType> = Id::from_static(3);
pub const PIGLIN_BRUTE_SPECIFIC_SENSOR: Id<crate::SensorType> = Id::from_static(12);
pub const PIGLIN_SPECIFIC_SENSOR: Id<crate::SensorType> = Id::from_static(11);
pub const SECONDARY_POIS: Id<crate::SensorType> = Id::from_static(8);
pub const VILLAGER_BABIES: Id<crate::SensorType> = Id::from_static(7);
pub const VILLAGER_HOSTILES: Id<crate::SensorType> = Id::from_static(6);
pub const WARDEN_ENTITY_SENSOR: Id<crate::SensorType> = Id::from_static(22);

pub const NAMES: &[&str] = &[
    "minecraft:dummy",
    "minecraft:nearest_items",
    "minecraft:nearest_living_entities",
    "minecraft:nearest_players",
    "minecraft:nearest_bed",
    "minecraft:hurt_by",
    "minecraft:villager_hostiles",
    "minecraft:villager_babies",
    "minecraft:secondary_pois",
    "minecraft:golem_detected",
    "minecraft:armadillo_scare_detected",
    "minecraft:piglin_specific_sensor",
    "minecraft:piglin_brute_specific_sensor",
    "minecraft:hoglin_specific_sensor",
    "minecraft:nearest_adult",
    "minecraft:nearest_adult_any_type",
    "minecraft:axolotl_attackables",
    "minecraft:food_temptations",
    "minecraft:frog_temptations",
    "minecraft:nautilus_temptations",
    "minecraft:frog_attackables",
    "minecraft:is_in_water",
    "minecraft:warden_entity_sensor",
    "minecraft:breeze_attack_entity_sensor",
];
