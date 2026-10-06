// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const DUMMY: StaticResourceLocation = rl!("minecraft:dummy");
pub const NEAREST_ITEMS: StaticResourceLocation = rl!("minecraft:nearest_items");
pub const NEAREST_LIVING_ENTITIES: StaticResourceLocation = rl!("minecraft:nearest_living_entities");
pub const NEAREST_PLAYERS: StaticResourceLocation = rl!("minecraft:nearest_players");
pub const NEAREST_BED: StaticResourceLocation = rl!("minecraft:nearest_bed");
pub const HURT_BY: StaticResourceLocation = rl!("minecraft:hurt_by");
pub const VILLAGER_HOSTILES: StaticResourceLocation = rl!("minecraft:villager_hostiles");
pub const VILLAGER_BABIES: StaticResourceLocation = rl!("minecraft:villager_babies");
pub const SECONDARY_POIS: StaticResourceLocation = rl!("minecraft:secondary_pois");
pub const GOLEM_DETECTED: StaticResourceLocation = rl!("minecraft:golem_detected");
pub const ARMADILLO_SCARE_DETECTED: StaticResourceLocation = rl!("minecraft:armadillo_scare_detected");
pub const PIGLIN_SPECIFIC_SENSOR: StaticResourceLocation = rl!("minecraft:piglin_specific_sensor");
pub const PIGLIN_BRUTE_SPECIFIC_SENSOR: StaticResourceLocation = rl!("minecraft:piglin_brute_specific_sensor");
pub const HOGLIN_SPECIFIC_SENSOR: StaticResourceLocation = rl!("minecraft:hoglin_specific_sensor");
pub const NEAREST_ADULT: StaticResourceLocation = rl!("minecraft:nearest_adult");
pub const NEAREST_ADULT_ANY_TYPE: StaticResourceLocation = rl!("minecraft:nearest_adult_any_type");
pub const AXOLOTL_ATTACKABLES: StaticResourceLocation = rl!("minecraft:axolotl_attackables");
pub const FOOD_TEMPTATIONS: StaticResourceLocation = rl!("minecraft:food_temptations");
pub const FROG_TEMPTATIONS: StaticResourceLocation = rl!("minecraft:frog_temptations");
pub const NAUTILUS_TEMPTATIONS: StaticResourceLocation = rl!("minecraft:nautilus_temptations");
pub const FROG_ATTACKABLES: StaticResourceLocation = rl!("minecraft:frog_attackables");
pub const IS_IN_WATER: StaticResourceLocation = rl!("minecraft:is_in_water");
pub const WARDEN_ENTITY_SENSOR: StaticResourceLocation = rl!("minecraft:warden_entity_sensor");
pub const BREEZE_ATTACK_ENTITY_SENSOR: StaticResourceLocation = rl!("minecraft:breeze_attack_entity_sensor");

pub const ENTRIES: &[StaticResourceLocation] = &[
    DUMMY,
    NEAREST_ITEMS,
    NEAREST_LIVING_ENTITIES,
    NEAREST_PLAYERS,
    NEAREST_BED,
    HURT_BY,
    VILLAGER_HOSTILES,
    VILLAGER_BABIES,
    SECONDARY_POIS,
    GOLEM_DETECTED,
    ARMADILLO_SCARE_DETECTED,
    PIGLIN_SPECIFIC_SENSOR,
    PIGLIN_BRUTE_SPECIFIC_SENSOR,
    HOGLIN_SPECIFIC_SENSOR,
    NEAREST_ADULT,
    NEAREST_ADULT_ANY_TYPE,
    AXOLOTL_ATTACKABLES,
    FOOD_TEMPTATIONS,
    FROG_TEMPTATIONS,
    NAUTILUS_TEMPTATIONS,
    FROG_ATTACKABLES,
    IS_IN_WATER,
    WARDEN_ENTITY_SENSOR,
    BREEZE_ATTACK_ENTITY_SENSOR,
];
