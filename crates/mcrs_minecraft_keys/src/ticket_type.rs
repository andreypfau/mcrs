// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const PLAYER_SPAWN: ResourceLocation<'static> = rl!("minecraft:player_spawn");
pub const SPAWN_SEARCH: ResourceLocation<'static> = rl!("minecraft:spawn_search");
pub const DRAGON: ResourceLocation<'static> = rl!("minecraft:dragon");
pub const PLAYER_LOADING: ResourceLocation<'static> = rl!("minecraft:player_loading");
pub const PLAYER_SIMULATION: ResourceLocation<'static> = rl!("minecraft:player_simulation");
pub const FORCED: ResourceLocation<'static> = rl!("minecraft:forced");
pub const PORTAL: ResourceLocation<'static> = rl!("minecraft:portal");
pub const ENDER_PEARL: ResourceLocation<'static> = rl!("minecraft:ender_pearl");
pub const UNKNOWN: ResourceLocation<'static> = rl!("minecraft:unknown");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
