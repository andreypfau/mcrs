// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const BOOLEAN: ResourceLocation<'static> = rl!("minecraft:boolean");
pub const TRI_STATE: ResourceLocation<'static> = rl!("minecraft:tri_state");
pub const FLOAT: ResourceLocation<'static> = rl!("minecraft:float");
pub const ANGLE_DEGREES: ResourceLocation<'static> = rl!("minecraft:angle_degrees");
pub const RGB_COLOR: ResourceLocation<'static> = rl!("minecraft:rgb_color");
pub const ARGB_COLOR: ResourceLocation<'static> = rl!("minecraft:argb_color");
pub const INTEGER: ResourceLocation<'static> = rl!("minecraft:integer");
pub const MOON_PHASE: ResourceLocation<'static> = rl!("minecraft:moon_phase");
pub const ACTIVITY: ResourceLocation<'static> = rl!("minecraft:activity");
pub const BED_RULE: ResourceLocation<'static> = rl!("minecraft:bed_rule");
pub const PARTICLE: ResourceLocation<'static> = rl!("minecraft:particle");
pub const AMBIENT_PARTICLES: ResourceLocation<'static> = rl!("minecraft:ambient_particles");
pub const BACKGROUND_MUSIC: ResourceLocation<'static> = rl!("minecraft:background_music");
pub const AMBIENT_SOUNDS: ResourceLocation<'static> = rl!("minecraft:ambient_sounds");
pub const MOB_SPAWN_SETTINGS: ResourceLocation<'static> = rl!("minecraft:mob_spawn_settings");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
