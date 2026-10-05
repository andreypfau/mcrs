// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BEE_HIVES: Id<crate::DebugSubscription> = Id::from_static(7);
pub const BEES: Id<crate::DebugSubscription> = Id::from_static(1);
pub const BRAINS: Id<crate::DebugSubscription> = Id::from_static(2);
pub const BREEZES: Id<crate::DebugSubscription> = Id::from_static(3);
pub const DEDICATED_SERVER_TICK_TIME: Id<crate::DebugSubscription> = Id::from_static(0);
pub const ENTITY_BLOCK_INTERSECTIONS: Id<crate::DebugSubscription> = Id::from_static(6);
pub const ENTITY_PATHS: Id<crate::DebugSubscription> = Id::from_static(5);
pub const GAME_EVENT_LISTENERS: Id<crate::DebugSubscription> = Id::from_static(13);
pub const GAME_EVENTS: Id<crate::DebugSubscription> = Id::from_static(15);
pub const GOAL_SELECTORS: Id<crate::DebugSubscription> = Id::from_static(4);
pub const NEIGHBOR_UPDATES: Id<crate::DebugSubscription> = Id::from_static(14);
pub const POIS: Id<crate::DebugSubscription> = Id::from_static(8);
pub const RAIDS: Id<crate::DebugSubscription> = Id::from_static(11);
pub const REDSTONE_WIRE_ORIENTATIONS: Id<crate::DebugSubscription> = Id::from_static(9);
pub const STRUCTURES: Id<crate::DebugSubscription> = Id::from_static(12);
pub const VILLAGE_SECTIONS: Id<crate::DebugSubscription> = Id::from_static(10);

pub const NAMES: &[&str] = &[
    "minecraft:dedicated_server_tick_time",
    "minecraft:bees",
    "minecraft:brains",
    "minecraft:breezes",
    "minecraft:goal_selectors",
    "minecraft:entity_paths",
    "minecraft:entity_block_intersections",
    "minecraft:bee_hives",
    "minecraft:pois",
    "minecraft:redstone_wire_orientations",
    "minecraft:village_sections",
    "minecraft:raids",
    "minecraft:structures",
    "minecraft:game_event_listeners",
    "minecraft:neighbor_updates",
    "minecraft:game_events",
];
