// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const BLOCK_ACTIVATE: ResourceLocation<'static> = rl!("minecraft:block_activate");
pub const BLOCK_ATTACH: ResourceLocation<'static> = rl!("minecraft:block_attach");
pub const BLOCK_CHANGE: ResourceLocation<'static> = rl!("minecraft:block_change");
pub const BLOCK_CLOSE: ResourceLocation<'static> = rl!("minecraft:block_close");
pub const BLOCK_DEACTIVATE: ResourceLocation<'static> = rl!("minecraft:block_deactivate");
pub const BLOCK_DESTROY: ResourceLocation<'static> = rl!("minecraft:block_destroy");
pub const BLOCK_DETACH: ResourceLocation<'static> = rl!("minecraft:block_detach");
pub const BLOCK_OPEN: ResourceLocation<'static> = rl!("minecraft:block_open");
pub const BLOCK_PLACE: ResourceLocation<'static> = rl!("minecraft:block_place");
pub const BOUNCE: ResourceLocation<'static> = rl!("minecraft:bounce");
pub const CONTAINER_CLOSE: ResourceLocation<'static> = rl!("minecraft:container_close");
pub const CONTAINER_OPEN: ResourceLocation<'static> = rl!("minecraft:container_open");
pub const DRINK: ResourceLocation<'static> = rl!("minecraft:drink");
pub const EAT: ResourceLocation<'static> = rl!("minecraft:eat");
pub const ELYTRA_GLIDE: ResourceLocation<'static> = rl!("minecraft:elytra_glide");
pub const ENTITY_DAMAGE: ResourceLocation<'static> = rl!("minecraft:entity_damage");
pub const ENTITY_DIE: ResourceLocation<'static> = rl!("minecraft:entity_die");
pub const ENTITY_DISMOUNT: ResourceLocation<'static> = rl!("minecraft:entity_dismount");
pub const ENTITY_INTERACT: ResourceLocation<'static> = rl!("minecraft:entity_interact");
pub const ENTITY_MOUNT: ResourceLocation<'static> = rl!("minecraft:entity_mount");
pub const ENTITY_PLACE: ResourceLocation<'static> = rl!("minecraft:entity_place");
pub const ENTITY_ACTION: ResourceLocation<'static> = rl!("minecraft:entity_action");
pub const EQUIP: ResourceLocation<'static> = rl!("minecraft:equip");
pub const EXPLODE: ResourceLocation<'static> = rl!("minecraft:explode");
pub const FLAP: ResourceLocation<'static> = rl!("minecraft:flap");
pub const FLUID_PICKUP: ResourceLocation<'static> = rl!("minecraft:fluid_pickup");
pub const FLUID_PLACE: ResourceLocation<'static> = rl!("minecraft:fluid_place");
pub const HIT_GROUND: ResourceLocation<'static> = rl!("minecraft:hit_ground");
pub const INSTRUMENT_PLAY: ResourceLocation<'static> = rl!("minecraft:instrument_play");
pub const ITEM_INTERACT_FINISH: ResourceLocation<'static> = rl!("minecraft:item_interact_finish");
pub const ITEM_INTERACT_START: ResourceLocation<'static> = rl!("minecraft:item_interact_start");
pub const JUKEBOX_PLAY: ResourceLocation<'static> = rl!("minecraft:jukebox_play");
pub const JUKEBOX_STOP_PLAY: ResourceLocation<'static> = rl!("minecraft:jukebox_stop_play");
pub const LIGHTNING_STRIKE: ResourceLocation<'static> = rl!("minecraft:lightning_strike");
pub const NOTE_BLOCK_PLAY: ResourceLocation<'static> = rl!("minecraft:note_block_play");
pub const PRIME_FUSE: ResourceLocation<'static> = rl!("minecraft:prime_fuse");
pub const PROJECTILE_LAND: ResourceLocation<'static> = rl!("minecraft:projectile_land");
pub const PROJECTILE_SHOOT: ResourceLocation<'static> = rl!("minecraft:projectile_shoot");
pub const SCULK_SENSOR_TENDRILS_CLICKING: ResourceLocation<'static> = rl!("minecraft:sculk_sensor_tendrils_clicking");
pub const SHEAR: ResourceLocation<'static> = rl!("minecraft:shear");
pub const SHRIEK: ResourceLocation<'static> = rl!("minecraft:shriek");
pub const SPLASH: ResourceLocation<'static> = rl!("minecraft:splash");
pub const STEP: ResourceLocation<'static> = rl!("minecraft:step");
pub const SWIM: ResourceLocation<'static> = rl!("minecraft:swim");
pub const TELEPORT: ResourceLocation<'static> = rl!("minecraft:teleport");
pub const UNEQUIP: ResourceLocation<'static> = rl!("minecraft:unequip");
pub const RESONATE_1: ResourceLocation<'static> = rl!("minecraft:resonate_1");
pub const RESONATE_2: ResourceLocation<'static> = rl!("minecraft:resonate_2");
pub const RESONATE_3: ResourceLocation<'static> = rl!("minecraft:resonate_3");
pub const RESONATE_4: ResourceLocation<'static> = rl!("minecraft:resonate_4");
pub const RESONATE_5: ResourceLocation<'static> = rl!("minecraft:resonate_5");
pub const RESONATE_6: ResourceLocation<'static> = rl!("minecraft:resonate_6");
pub const RESONATE_7: ResourceLocation<'static> = rl!("minecraft:resonate_7");
pub const RESONATE_8: ResourceLocation<'static> = rl!("minecraft:resonate_8");
pub const RESONATE_9: ResourceLocation<'static> = rl!("minecraft:resonate_9");
pub const RESONATE_10: ResourceLocation<'static> = rl!("minecraft:resonate_10");
pub const RESONATE_11: ResourceLocation<'static> = rl!("minecraft:resonate_11");
pub const RESONATE_12: ResourceLocation<'static> = rl!("minecraft:resonate_12");
pub const RESONATE_13: ResourceLocation<'static> = rl!("minecraft:resonate_13");
pub const RESONATE_14: ResourceLocation<'static> = rl!("minecraft:resonate_14");
pub const RESONATE_15: ResourceLocation<'static> = rl!("minecraft:resonate_15");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    BLOCK_ACTIVATE,
    BLOCK_ATTACH,
    BLOCK_CHANGE,
    BLOCK_CLOSE,
    BLOCK_DEACTIVATE,
    BLOCK_DESTROY,
    BLOCK_DETACH,
    BLOCK_OPEN,
    BLOCK_PLACE,
    BOUNCE,
    CONTAINER_CLOSE,
    CONTAINER_OPEN,
    DRINK,
    EAT,
    ELYTRA_GLIDE,
    ENTITY_DAMAGE,
    ENTITY_DIE,
    ENTITY_DISMOUNT,
    ENTITY_INTERACT,
    ENTITY_MOUNT,
    ENTITY_PLACE,
    ENTITY_ACTION,
    EQUIP,
    EXPLODE,
    FLAP,
    FLUID_PICKUP,
    FLUID_PLACE,
    HIT_GROUND,
    INSTRUMENT_PLAY,
    ITEM_INTERACT_FINISH,
    ITEM_INTERACT_START,
    JUKEBOX_PLAY,
    JUKEBOX_STOP_PLAY,
    LIGHTNING_STRIKE,
    NOTE_BLOCK_PLAY,
    PRIME_FUSE,
    PROJECTILE_LAND,
    PROJECTILE_SHOOT,
    SCULK_SENSOR_TENDRILS_CLICKING,
    SHEAR,
    SHRIEK,
    SPLASH,
    STEP,
    SWIM,
    TELEPORT,
    UNEQUIP,
    RESONATE_1,
    RESONATE_2,
    RESONATE_3,
    RESONATE_4,
    RESONATE_5,
    RESONATE_6,
    RESONATE_7,
    RESONATE_8,
    RESONATE_9,
    RESONATE_10,
    RESONATE_11,
    RESONATE_12,
    RESONATE_13,
    RESONATE_14,
    RESONATE_15,
];
