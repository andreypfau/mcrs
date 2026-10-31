// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const DEDICATED_SERVER_TICK_TIME: ResourceLocation<'static> = rl!("minecraft:dedicated_server_tick_time");
pub const BEES: ResourceLocation<'static> = rl!("minecraft:bees");
pub const BRAINS: ResourceLocation<'static> = rl!("minecraft:brains");
pub const BREEZES: ResourceLocation<'static> = rl!("minecraft:breezes");
pub const GOAL_SELECTORS: ResourceLocation<'static> = rl!("minecraft:goal_selectors");
pub const ENTITY_PATHS: ResourceLocation<'static> = rl!("minecraft:entity_paths");
pub const ENTITY_BLOCK_INTERSECTIONS: ResourceLocation<'static> = rl!("minecraft:entity_block_intersections");
pub const BEE_HIVES: ResourceLocation<'static> = rl!("minecraft:bee_hives");
pub const POIS: ResourceLocation<'static> = rl!("minecraft:pois");
pub const REDSTONE_WIRE_ORIENTATIONS: ResourceLocation<'static> = rl!("minecraft:redstone_wire_orientations");
pub const VILLAGE_SECTIONS: ResourceLocation<'static> = rl!("minecraft:village_sections");
pub const RAIDS: ResourceLocation<'static> = rl!("minecraft:raids");
pub const STRUCTURES: ResourceLocation<'static> = rl!("minecraft:structures");
pub const GAME_EVENT_LISTENERS: ResourceLocation<'static> = rl!("minecraft:game_event_listeners");
pub const NEIGHBOR_UPDATES: ResourceLocation<'static> = rl!("minecraft:neighbor_updates");
pub const GAME_EVENTS: ResourceLocation<'static> = rl!("minecraft:game_events");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
