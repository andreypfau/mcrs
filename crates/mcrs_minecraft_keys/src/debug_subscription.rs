// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const DEDICATED_SERVER_TICK_TIME: StaticResourceLocation = rl!("minecraft:dedicated_server_tick_time");
pub const BEES: StaticResourceLocation = rl!("minecraft:bees");
pub const BRAINS: StaticResourceLocation = rl!("minecraft:brains");
pub const BREEZES: StaticResourceLocation = rl!("minecraft:breezes");
pub const GOAL_SELECTORS: StaticResourceLocation = rl!("minecraft:goal_selectors");
pub const ENTITY_PATHS: StaticResourceLocation = rl!("minecraft:entity_paths");
pub const ENTITY_BLOCK_INTERSECTIONS: StaticResourceLocation = rl!("minecraft:entity_block_intersections");
pub const BEE_HIVES: StaticResourceLocation = rl!("minecraft:bee_hives");
pub const POIS: StaticResourceLocation = rl!("minecraft:pois");
pub const REDSTONE_WIRE_ORIENTATIONS: StaticResourceLocation = rl!("minecraft:redstone_wire_orientations");
pub const VILLAGE_SECTIONS: StaticResourceLocation = rl!("minecraft:village_sections");
pub const RAIDS: StaticResourceLocation = rl!("minecraft:raids");
pub const STRUCTURES: StaticResourceLocation = rl!("minecraft:structures");
pub const GAME_EVENT_LISTENERS: StaticResourceLocation = rl!("minecraft:game_event_listeners");
pub const NEIGHBOR_UPDATES: StaticResourceLocation = rl!("minecraft:neighbor_updates");
pub const GAME_EVENTS: StaticResourceLocation = rl!("minecraft:game_events");

pub const ENTRIES: &[StaticResourceLocation] = &[
    DEDICATED_SERVER_TICK_TIME,
    BEES,
    BRAINS,
    BREEZES,
    GOAL_SELECTORS,
    ENTITY_PATHS,
    ENTITY_BLOCK_INTERSECTIONS,
    BEE_HIVES,
    POIS,
    REDSTONE_WIRE_ORIENTATIONS,
    VILLAGE_SECTIONS,
    RAIDS,
    STRUCTURES,
    GAME_EVENT_LISTENERS,
    NEIGHBOR_UPDATES,
    GAME_EVENTS,
];
