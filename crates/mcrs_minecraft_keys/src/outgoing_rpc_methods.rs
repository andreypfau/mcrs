// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_keys! {
    crate::OutgoingRpcMethods;
    NOTIFICATION_SERVER_STARTED = "minecraft:notification/server/started",
    NOTIFICATION_SERVER_STOPPING = "minecraft:notification/server/stopping",
    NOTIFICATION_SERVER_SAVING = "minecraft:notification/server/saving",
    NOTIFICATION_SERVER_SAVED = "minecraft:notification/server/saved",
    NOTIFICATION_SERVER_ACTIVITY = "minecraft:notification/server/activity",
    NOTIFICATION_WORLD_UPGRADE_STARTED = "minecraft:notification/world/upgrade_started",
    NOTIFICATION_WORLD_UPGRADE_PROGRESS = "minecraft:notification/world/upgrade_progress",
    NOTIFICATION_WORLD_UPGRADE_FINISHED = "minecraft:notification/world/upgrade_finished",
    NOTIFICATION_WORLD_UPGRADE_FAILED = "minecraft:notification/world/upgrade_failed",
    NOTIFICATION_PLAYERS_JOINED = "minecraft:notification/players/joined",
    NOTIFICATION_PLAYERS_LEFT = "minecraft:notification/players/left",
    NOTIFICATION_OPERATORS_ADDED = "minecraft:notification/operators/added",
    NOTIFICATION_OPERATORS_REMOVED = "minecraft:notification/operators/removed",
    NOTIFICATION_ALLOWLIST_ADDED = "minecraft:notification/allowlist/added",
    NOTIFICATION_ALLOWLIST_REMOVED = "minecraft:notification/allowlist/removed",
    NOTIFICATION_IP_BANS_ADDED = "minecraft:notification/ip_bans/added",
    NOTIFICATION_IP_BANS_REMOVED = "minecraft:notification/ip_bans/removed",
    NOTIFICATION_BANS_ADDED = "minecraft:notification/bans/added",
    NOTIFICATION_BANS_REMOVED = "minecraft:notification/bans/removed",
    NOTIFICATION_GAMERULES_UPDATED = "minecraft:notification/gamerules/updated",
    NOTIFICATION_SERVER_STATUS = "minecraft:notification/server/status",
}
