// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const PLAYER_SPAWN: StaticResourceLocation = rl!("minecraft:player_spawn");
pub const SPAWN_SEARCH: StaticResourceLocation = rl!("minecraft:spawn_search");
pub const DRAGON: StaticResourceLocation = rl!("minecraft:dragon");
pub const PLAYER_LOADING: StaticResourceLocation = rl!("minecraft:player_loading");
pub const PLAYER_SIMULATION: StaticResourceLocation = rl!("minecraft:player_simulation");
pub const FORCED: StaticResourceLocation = rl!("minecraft:forced");
pub const PORTAL: StaticResourceLocation = rl!("minecraft:portal");
pub const ENDER_PEARL: StaticResourceLocation = rl!("minecraft:ender_pearl");
pub const UNKNOWN: StaticResourceLocation = rl!("minecraft:unknown");

pub const ENTRIES: &[StaticResourceLocation] = &[
    PLAYER_SPAWN,
    SPAWN_SEARCH,
    DRAGON,
    PLAYER_LOADING,
    PLAYER_SIMULATION,
    FORCED,
    PORTAL,
    ENDER_PEARL,
    UNKNOWN,
];
