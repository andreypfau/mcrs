// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const NOTIFICATION_ALLOWLIST_ADDED: Id<crate::OutgoingRpcMethods> = Id::from_static(13);
pub const NOTIFICATION_ALLOWLIST_REMOVED: Id<crate::OutgoingRpcMethods> = Id::from_static(14);
pub const NOTIFICATION_BANS_ADDED: Id<crate::OutgoingRpcMethods> = Id::from_static(17);
pub const NOTIFICATION_BANS_REMOVED: Id<crate::OutgoingRpcMethods> = Id::from_static(18);
pub const NOTIFICATION_GAMERULES_UPDATED: Id<crate::OutgoingRpcMethods> = Id::from_static(19);
pub const NOTIFICATION_IP_BANS_ADDED: Id<crate::OutgoingRpcMethods> = Id::from_static(15);
pub const NOTIFICATION_IP_BANS_REMOVED: Id<crate::OutgoingRpcMethods> = Id::from_static(16);
pub const NOTIFICATION_OPERATORS_ADDED: Id<crate::OutgoingRpcMethods> = Id::from_static(11);
pub const NOTIFICATION_OPERATORS_REMOVED: Id<crate::OutgoingRpcMethods> = Id::from_static(12);
pub const NOTIFICATION_PLAYERS_JOINED: Id<crate::OutgoingRpcMethods> = Id::from_static(9);
pub const NOTIFICATION_PLAYERS_LEFT: Id<crate::OutgoingRpcMethods> = Id::from_static(10);
pub const NOTIFICATION_SERVER_ACTIVITY: Id<crate::OutgoingRpcMethods> = Id::from_static(4);
pub const NOTIFICATION_SERVER_SAVED: Id<crate::OutgoingRpcMethods> = Id::from_static(3);
pub const NOTIFICATION_SERVER_SAVING: Id<crate::OutgoingRpcMethods> = Id::from_static(2);
pub const NOTIFICATION_SERVER_STARTED: Id<crate::OutgoingRpcMethods> = Id::from_static(0);
pub const NOTIFICATION_SERVER_STATUS: Id<crate::OutgoingRpcMethods> = Id::from_static(20);
pub const NOTIFICATION_SERVER_STOPPING: Id<crate::OutgoingRpcMethods> = Id::from_static(1);
pub const NOTIFICATION_WORLD_UPGRADE_FAILED: Id<crate::OutgoingRpcMethods> = Id::from_static(8);
pub const NOTIFICATION_WORLD_UPGRADE_FINISHED: Id<crate::OutgoingRpcMethods> = Id::from_static(7);
pub const NOTIFICATION_WORLD_UPGRADE_PROGRESS: Id<crate::OutgoingRpcMethods> = Id::from_static(6);
pub const NOTIFICATION_WORLD_UPGRADE_STARTED: Id<crate::OutgoingRpcMethods> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:notification/server/started",
    "minecraft:notification/server/stopping",
    "minecraft:notification/server/saving",
    "minecraft:notification/server/saved",
    "minecraft:notification/server/activity",
    "minecraft:notification/world/upgrade_started",
    "minecraft:notification/world/upgrade_progress",
    "minecraft:notification/world/upgrade_finished",
    "minecraft:notification/world/upgrade_failed",
    "minecraft:notification/players/joined",
    "minecraft:notification/players/left",
    "minecraft:notification/operators/added",
    "minecraft:notification/operators/removed",
    "minecraft:notification/allowlist/added",
    "minecraft:notification/allowlist/removed",
    "minecraft:notification/ip_bans/added",
    "minecraft:notification/ip_bans/removed",
    "minecraft:notification/bans/added",
    "minecraft:notification/bans/removed",
    "minecraft:notification/gamerules/updated",
    "minecraft:notification/server/status",
];
