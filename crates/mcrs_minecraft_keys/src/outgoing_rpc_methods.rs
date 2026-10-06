// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const NOTIFICATION_SERVER_STARTED: StaticResourceLocation = rl!("minecraft:notification/server/started");
pub const NOTIFICATION_SERVER_STOPPING: StaticResourceLocation = rl!("minecraft:notification/server/stopping");
pub const NOTIFICATION_SERVER_SAVING: StaticResourceLocation = rl!("minecraft:notification/server/saving");
pub const NOTIFICATION_SERVER_SAVED: StaticResourceLocation = rl!("minecraft:notification/server/saved");
pub const NOTIFICATION_SERVER_ACTIVITY: StaticResourceLocation = rl!("minecraft:notification/server/activity");
pub const NOTIFICATION_WORLD_UPGRADE_STARTED: StaticResourceLocation = rl!("minecraft:notification/world/upgrade_started");
pub const NOTIFICATION_WORLD_UPGRADE_PROGRESS: StaticResourceLocation = rl!("minecraft:notification/world/upgrade_progress");
pub const NOTIFICATION_WORLD_UPGRADE_FINISHED: StaticResourceLocation = rl!("minecraft:notification/world/upgrade_finished");
pub const NOTIFICATION_WORLD_UPGRADE_FAILED: StaticResourceLocation = rl!("minecraft:notification/world/upgrade_failed");
pub const NOTIFICATION_PLAYERS_JOINED: StaticResourceLocation = rl!("minecraft:notification/players/joined");
pub const NOTIFICATION_PLAYERS_LEFT: StaticResourceLocation = rl!("minecraft:notification/players/left");
pub const NOTIFICATION_OPERATORS_ADDED: StaticResourceLocation = rl!("minecraft:notification/operators/added");
pub const NOTIFICATION_OPERATORS_REMOVED: StaticResourceLocation = rl!("minecraft:notification/operators/removed");
pub const NOTIFICATION_ALLOWLIST_ADDED: StaticResourceLocation = rl!("minecraft:notification/allowlist/added");
pub const NOTIFICATION_ALLOWLIST_REMOVED: StaticResourceLocation = rl!("minecraft:notification/allowlist/removed");
pub const NOTIFICATION_IP_BANS_ADDED: StaticResourceLocation = rl!("minecraft:notification/ip_bans/added");
pub const NOTIFICATION_IP_BANS_REMOVED: StaticResourceLocation = rl!("minecraft:notification/ip_bans/removed");
pub const NOTIFICATION_BANS_ADDED: StaticResourceLocation = rl!("minecraft:notification/bans/added");
pub const NOTIFICATION_BANS_REMOVED: StaticResourceLocation = rl!("minecraft:notification/bans/removed");
pub const NOTIFICATION_GAMERULES_UPDATED: StaticResourceLocation = rl!("minecraft:notification/gamerules/updated");
pub const NOTIFICATION_SERVER_STATUS: StaticResourceLocation = rl!("minecraft:notification/server/status");

pub const ENTRIES: &[StaticResourceLocation] = &[
    NOTIFICATION_SERVER_STARTED,
    NOTIFICATION_SERVER_STOPPING,
    NOTIFICATION_SERVER_SAVING,
    NOTIFICATION_SERVER_SAVED,
    NOTIFICATION_SERVER_ACTIVITY,
    NOTIFICATION_WORLD_UPGRADE_STARTED,
    NOTIFICATION_WORLD_UPGRADE_PROGRESS,
    NOTIFICATION_WORLD_UPGRADE_FINISHED,
    NOTIFICATION_WORLD_UPGRADE_FAILED,
    NOTIFICATION_PLAYERS_JOINED,
    NOTIFICATION_PLAYERS_LEFT,
    NOTIFICATION_OPERATORS_ADDED,
    NOTIFICATION_OPERATORS_REMOVED,
    NOTIFICATION_ALLOWLIST_ADDED,
    NOTIFICATION_ALLOWLIST_REMOVED,
    NOTIFICATION_IP_BANS_ADDED,
    NOTIFICATION_IP_BANS_REMOVED,
    NOTIFICATION_BANS_ADDED,
    NOTIFICATION_BANS_REMOVED,
    NOTIFICATION_GAMERULES_UPDATED,
    NOTIFICATION_SERVER_STATUS,
];
