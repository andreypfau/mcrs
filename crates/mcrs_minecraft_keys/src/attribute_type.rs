// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BOOLEAN: StaticResourceLocation = rl!("minecraft:boolean");
pub const TRI_STATE: StaticResourceLocation = rl!("minecraft:tri_state");
pub const FLOAT: StaticResourceLocation = rl!("minecraft:float");
pub const ANGLE_DEGREES: StaticResourceLocation = rl!("minecraft:angle_degrees");
pub const RGB_COLOR: StaticResourceLocation = rl!("minecraft:rgb_color");
pub const ARGB_COLOR: StaticResourceLocation = rl!("minecraft:argb_color");
pub const INTEGER: StaticResourceLocation = rl!("minecraft:integer");
pub const MOON_PHASE: StaticResourceLocation = rl!("minecraft:moon_phase");
pub const ACTIVITY: StaticResourceLocation = rl!("minecraft:activity");
pub const BED_RULE: StaticResourceLocation = rl!("minecraft:bed_rule");
pub const PARTICLE: StaticResourceLocation = rl!("minecraft:particle");
pub const AMBIENT_PARTICLES: StaticResourceLocation = rl!("minecraft:ambient_particles");
pub const BACKGROUND_MUSIC: StaticResourceLocation = rl!("minecraft:background_music");
pub const AMBIENT_SOUNDS: StaticResourceLocation = rl!("minecraft:ambient_sounds");
pub const MOB_SPAWN_SETTINGS: StaticResourceLocation = rl!("minecraft:mob_spawn_settings");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BOOLEAN,
    TRI_STATE,
    FLOAT,
    ANGLE_DEGREES,
    RGB_COLOR,
    ARGB_COLOR,
    INTEGER,
    MOON_PHASE,
    ACTIVITY,
    BED_RULE,
    PARTICLE,
    AMBIENT_PARTICLES,
    BACKGROUND_MUSIC,
    AMBIENT_SOUNDS,
    MOB_SPAWN_SETTINGS,
];
