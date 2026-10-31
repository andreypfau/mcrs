// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const DUMMY: ResourceLocation<'static> = rl!("minecraft:dummy");
pub const NEAREST_ITEMS: ResourceLocation<'static> = rl!("minecraft:nearest_items");
pub const NEAREST_LIVING_ENTITIES: ResourceLocation<'static> = rl!("minecraft:nearest_living_entities");
pub const NEAREST_PLAYERS: ResourceLocation<'static> = rl!("minecraft:nearest_players");
pub const NEAREST_BED: ResourceLocation<'static> = rl!("minecraft:nearest_bed");
pub const HURT_BY: ResourceLocation<'static> = rl!("minecraft:hurt_by");
pub const VILLAGER_HOSTILES: ResourceLocation<'static> = rl!("minecraft:villager_hostiles");
pub const VILLAGER_BABIES: ResourceLocation<'static> = rl!("minecraft:villager_babies");
pub const SECONDARY_POIS: ResourceLocation<'static> = rl!("minecraft:secondary_pois");
pub const GOLEM_DETECTED: ResourceLocation<'static> = rl!("minecraft:golem_detected");
pub const ARMADILLO_SCARE_DETECTED: ResourceLocation<'static> = rl!("minecraft:armadillo_scare_detected");
pub const PIGLIN_SPECIFIC_SENSOR: ResourceLocation<'static> = rl!("minecraft:piglin_specific_sensor");
pub const PIGLIN_BRUTE_SPECIFIC_SENSOR: ResourceLocation<'static> = rl!("minecraft:piglin_brute_specific_sensor");
pub const HOGLIN_SPECIFIC_SENSOR: ResourceLocation<'static> = rl!("minecraft:hoglin_specific_sensor");
pub const NEAREST_ADULT: ResourceLocation<'static> = rl!("minecraft:nearest_adult");
pub const NEAREST_ADULT_ANY_TYPE: ResourceLocation<'static> = rl!("minecraft:nearest_adult_any_type");
pub const AXOLOTL_ATTACKABLES: ResourceLocation<'static> = rl!("minecraft:axolotl_attackables");
pub const FOOD_TEMPTATIONS: ResourceLocation<'static> = rl!("minecraft:food_temptations");
pub const FROG_TEMPTATIONS: ResourceLocation<'static> = rl!("minecraft:frog_temptations");
pub const NAUTILUS_TEMPTATIONS: ResourceLocation<'static> = rl!("minecraft:nautilus_temptations");
pub const FROG_ATTACKABLES: ResourceLocation<'static> = rl!("minecraft:frog_attackables");
pub const IS_IN_WATER: ResourceLocation<'static> = rl!("minecraft:is_in_water");
pub const WARDEN_ENTITY_SENSOR: ResourceLocation<'static> = rl!("minecraft:warden_entity_sensor");
pub const BREEZE_ATTACK_ENTITY_SENSOR: ResourceLocation<'static> = rl!("minecraft:breeze_attack_entity_sensor");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
