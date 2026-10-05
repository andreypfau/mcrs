// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const DRAGON: Id<crate::TicketType> = Id::from_static(2);
pub const ENDER_PEARL: Id<crate::TicketType> = Id::from_static(7);
pub const FORCED: Id<crate::TicketType> = Id::from_static(5);
pub const PLAYER_LOADING: Id<crate::TicketType> = Id::from_static(3);
pub const PLAYER_SIMULATION: Id<crate::TicketType> = Id::from_static(4);
pub const PLAYER_SPAWN: Id<crate::TicketType> = Id::from_static(0);
pub const PORTAL: Id<crate::TicketType> = Id::from_static(6);
pub const SPAWN_SEARCH: Id<crate::TicketType> = Id::from_static(1);
pub const UNKNOWN: Id<crate::TicketType> = Id::from_static(8);

pub const NAMES: &[&str] = &[
    "minecraft:player_spawn",
    "minecraft:spawn_search",
    "minecraft:dragon",
    "minecraft:player_loading",
    "minecraft:player_simulation",
    "minecraft:forced",
    "minecraft:portal",
    "minecraft:ender_pearl",
    "minecraft:unknown",
];
