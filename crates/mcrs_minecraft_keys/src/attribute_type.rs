// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ACTIVITY: Id<crate::AttributeType> = Id::from_static(8);
pub const AMBIENT_PARTICLES: Id<crate::AttributeType> = Id::from_static(11);
pub const AMBIENT_SOUNDS: Id<crate::AttributeType> = Id::from_static(13);
pub const ANGLE_DEGREES: Id<crate::AttributeType> = Id::from_static(3);
pub const ARGB_COLOR: Id<crate::AttributeType> = Id::from_static(5);
pub const BACKGROUND_MUSIC: Id<crate::AttributeType> = Id::from_static(12);
pub const BED_RULE: Id<crate::AttributeType> = Id::from_static(9);
pub const BOOLEAN: Id<crate::AttributeType> = Id::from_static(0);
pub const FLOAT: Id<crate::AttributeType> = Id::from_static(2);
pub const INTEGER: Id<crate::AttributeType> = Id::from_static(6);
pub const MOB_SPAWN_SETTINGS: Id<crate::AttributeType> = Id::from_static(14);
pub const MOON_PHASE: Id<crate::AttributeType> = Id::from_static(7);
pub const PARTICLE: Id<crate::AttributeType> = Id::from_static(10);
pub const RGB_COLOR: Id<crate::AttributeType> = Id::from_static(4);
pub const TRI_STATE: Id<crate::AttributeType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:boolean",
    "minecraft:tri_state",
    "minecraft:float",
    "minecraft:angle_degrees",
    "minecraft:rgb_color",
    "minecraft:argb_color",
    "minecraft:integer",
    "minecraft:moon_phase",
    "minecraft:activity",
    "minecraft:bed_rule",
    "minecraft:particle",
    "minecraft:ambient_particles",
    "minecraft:background_music",
    "minecraft:ambient_sounds",
    "minecraft:mob_spawn_settings",
];
