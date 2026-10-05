// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BLOCK_ACTIVATE: Id<crate::GameEvent> = Id::from_static(0);
pub const BLOCK_ATTACH: Id<crate::GameEvent> = Id::from_static(1);
pub const BLOCK_CHANGE: Id<crate::GameEvent> = Id::from_static(2);
pub const BLOCK_CLOSE: Id<crate::GameEvent> = Id::from_static(3);
pub const BLOCK_DEACTIVATE: Id<crate::GameEvent> = Id::from_static(4);
pub const BLOCK_DESTROY: Id<crate::GameEvent> = Id::from_static(5);
pub const BLOCK_DETACH: Id<crate::GameEvent> = Id::from_static(6);
pub const BLOCK_OPEN: Id<crate::GameEvent> = Id::from_static(7);
pub const BLOCK_PLACE: Id<crate::GameEvent> = Id::from_static(8);
pub const BOUNCE: Id<crate::GameEvent> = Id::from_static(9);
pub const CONTAINER_CLOSE: Id<crate::GameEvent> = Id::from_static(10);
pub const CONTAINER_OPEN: Id<crate::GameEvent> = Id::from_static(11);
pub const DRINK: Id<crate::GameEvent> = Id::from_static(12);
pub const EAT: Id<crate::GameEvent> = Id::from_static(13);
pub const ELYTRA_GLIDE: Id<crate::GameEvent> = Id::from_static(14);
pub const ENTITY_ACTION: Id<crate::GameEvent> = Id::from_static(21);
pub const ENTITY_DAMAGE: Id<crate::GameEvent> = Id::from_static(15);
pub const ENTITY_DIE: Id<crate::GameEvent> = Id::from_static(16);
pub const ENTITY_DISMOUNT: Id<crate::GameEvent> = Id::from_static(17);
pub const ENTITY_INTERACT: Id<crate::GameEvent> = Id::from_static(18);
pub const ENTITY_MOUNT: Id<crate::GameEvent> = Id::from_static(19);
pub const ENTITY_PLACE: Id<crate::GameEvent> = Id::from_static(20);
pub const EQUIP: Id<crate::GameEvent> = Id::from_static(22);
pub const EXPLODE: Id<crate::GameEvent> = Id::from_static(23);
pub const FLAP: Id<crate::GameEvent> = Id::from_static(24);
pub const FLUID_PICKUP: Id<crate::GameEvent> = Id::from_static(25);
pub const FLUID_PLACE: Id<crate::GameEvent> = Id::from_static(26);
pub const HIT_GROUND: Id<crate::GameEvent> = Id::from_static(27);
pub const INSTRUMENT_PLAY: Id<crate::GameEvent> = Id::from_static(28);
pub const ITEM_INTERACT_FINISH: Id<crate::GameEvent> = Id::from_static(29);
pub const ITEM_INTERACT_START: Id<crate::GameEvent> = Id::from_static(30);
pub const JUKEBOX_PLAY: Id<crate::GameEvent> = Id::from_static(31);
pub const JUKEBOX_STOP_PLAY: Id<crate::GameEvent> = Id::from_static(32);
pub const LIGHTNING_STRIKE: Id<crate::GameEvent> = Id::from_static(33);
pub const NOTE_BLOCK_PLAY: Id<crate::GameEvent> = Id::from_static(34);
pub const PRIME_FUSE: Id<crate::GameEvent> = Id::from_static(35);
pub const PROJECTILE_LAND: Id<crate::GameEvent> = Id::from_static(36);
pub const PROJECTILE_SHOOT: Id<crate::GameEvent> = Id::from_static(37);
pub const RESONATE_1: Id<crate::GameEvent> = Id::from_static(46);
pub const RESONATE_10: Id<crate::GameEvent> = Id::from_static(55);
pub const RESONATE_11: Id<crate::GameEvent> = Id::from_static(56);
pub const RESONATE_12: Id<crate::GameEvent> = Id::from_static(57);
pub const RESONATE_13: Id<crate::GameEvent> = Id::from_static(58);
pub const RESONATE_14: Id<crate::GameEvent> = Id::from_static(59);
pub const RESONATE_15: Id<crate::GameEvent> = Id::from_static(60);
pub const RESONATE_2: Id<crate::GameEvent> = Id::from_static(47);
pub const RESONATE_3: Id<crate::GameEvent> = Id::from_static(48);
pub const RESONATE_4: Id<crate::GameEvent> = Id::from_static(49);
pub const RESONATE_5: Id<crate::GameEvent> = Id::from_static(50);
pub const RESONATE_6: Id<crate::GameEvent> = Id::from_static(51);
pub const RESONATE_7: Id<crate::GameEvent> = Id::from_static(52);
pub const RESONATE_8: Id<crate::GameEvent> = Id::from_static(53);
pub const RESONATE_9: Id<crate::GameEvent> = Id::from_static(54);
pub const SCULK_SENSOR_TENDRILS_CLICKING: Id<crate::GameEvent> = Id::from_static(38);
pub const SHEAR: Id<crate::GameEvent> = Id::from_static(39);
pub const SHRIEK: Id<crate::GameEvent> = Id::from_static(40);
pub const SPLASH: Id<crate::GameEvent> = Id::from_static(41);
pub const STEP: Id<crate::GameEvent> = Id::from_static(42);
pub const SWIM: Id<crate::GameEvent> = Id::from_static(43);
pub const TELEPORT: Id<crate::GameEvent> = Id::from_static(44);
pub const UNEQUIP: Id<crate::GameEvent> = Id::from_static(45);

pub const NAMES: &[&str] = &[
    "minecraft:block_activate",
    "minecraft:block_attach",
    "minecraft:block_change",
    "minecraft:block_close",
    "minecraft:block_deactivate",
    "minecraft:block_destroy",
    "minecraft:block_detach",
    "minecraft:block_open",
    "minecraft:block_place",
    "minecraft:bounce",
    "minecraft:container_close",
    "minecraft:container_open",
    "minecraft:drink",
    "minecraft:eat",
    "minecraft:elytra_glide",
    "minecraft:entity_damage",
    "minecraft:entity_die",
    "minecraft:entity_dismount",
    "minecraft:entity_interact",
    "minecraft:entity_mount",
    "minecraft:entity_place",
    "minecraft:entity_action",
    "minecraft:equip",
    "minecraft:explode",
    "minecraft:flap",
    "minecraft:fluid_pickup",
    "minecraft:fluid_place",
    "minecraft:hit_ground",
    "minecraft:instrument_play",
    "minecraft:item_interact_finish",
    "minecraft:item_interact_start",
    "minecraft:jukebox_play",
    "minecraft:jukebox_stop_play",
    "minecraft:lightning_strike",
    "minecraft:note_block_play",
    "minecraft:prime_fuse",
    "minecraft:projectile_land",
    "minecraft:projectile_shoot",
    "minecraft:sculk_sensor_tendrils_clicking",
    "minecraft:shear",
    "minecraft:shriek",
    "minecraft:splash",
    "minecraft:step",
    "minecraft:swim",
    "minecraft:teleport",
    "minecraft:unequip",
    "minecraft:resonate_1",
    "minecraft:resonate_2",
    "minecraft:resonate_3",
    "minecraft:resonate_4",
    "minecraft:resonate_5",
    "minecraft:resonate_6",
    "minecraft:resonate_7",
    "minecraft:resonate_8",
    "minecraft:resonate_9",
    "minecraft:resonate_10",
    "minecraft:resonate_11",
    "minecraft:resonate_12",
    "minecraft:resonate_13",
    "minecraft:resonate_14",
    "minecraft:resonate_15",
];
