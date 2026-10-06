// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const ALLOWLIST: StaticResourceLocation = rl!("minecraft:allowlist");
pub const ALLOWLIST_SET: StaticResourceLocation = rl!("minecraft:allowlist/set");
pub const ALLOWLIST_ADD: StaticResourceLocation = rl!("minecraft:allowlist/add");
pub const ALLOWLIST_REMOVE: StaticResourceLocation = rl!("minecraft:allowlist/remove");
pub const ALLOWLIST_CLEAR: StaticResourceLocation = rl!("minecraft:allowlist/clear");
pub const BANS: StaticResourceLocation = rl!("minecraft:bans");
pub const BANS_SET: StaticResourceLocation = rl!("minecraft:bans/set");
pub const BANS_ADD: StaticResourceLocation = rl!("minecraft:bans/add");
pub const BANS_REMOVE: StaticResourceLocation = rl!("minecraft:bans/remove");
pub const BANS_CLEAR: StaticResourceLocation = rl!("minecraft:bans/clear");
pub const IP_BANS: StaticResourceLocation = rl!("minecraft:ip_bans");
pub const IP_BANS_SET: StaticResourceLocation = rl!("minecraft:ip_bans/set");
pub const IP_BANS_ADD: StaticResourceLocation = rl!("minecraft:ip_bans/add");
pub const IP_BANS_REMOVE: StaticResourceLocation = rl!("minecraft:ip_bans/remove");
pub const IP_BANS_CLEAR: StaticResourceLocation = rl!("minecraft:ip_bans/clear");
pub const PLAYERS: StaticResourceLocation = rl!("minecraft:players");
pub const PLAYERS_KICK: StaticResourceLocation = rl!("minecraft:players/kick");
pub const OPERATORS: StaticResourceLocation = rl!("minecraft:operators");
pub const OPERATORS_SET: StaticResourceLocation = rl!("minecraft:operators/set");
pub const OPERATORS_ADD: StaticResourceLocation = rl!("minecraft:operators/add");
pub const OPERATORS_REMOVE: StaticResourceLocation = rl!("minecraft:operators/remove");
pub const OPERATORS_CLEAR: StaticResourceLocation = rl!("minecraft:operators/clear");
pub const SERVER_STATUS: StaticResourceLocation = rl!("minecraft:server/status");
pub const SERVER_SAVE: StaticResourceLocation = rl!("minecraft:server/save");
pub const SERVER_STOP: StaticResourceLocation = rl!("minecraft:server/stop");
pub const SERVER_SYSTEM_MESSAGE: StaticResourceLocation = rl!("minecraft:server/system_message");
pub const SERVERSETTINGS_AUTOSAVE: StaticResourceLocation = rl!("minecraft:serversettings/autosave");
pub const SERVERSETTINGS_AUTOSAVE_SET: StaticResourceLocation = rl!("minecraft:serversettings/autosave/set");
pub const SERVERSETTINGS_DIFFICULTY: StaticResourceLocation = rl!("minecraft:serversettings/difficulty");
pub const SERVERSETTINGS_DIFFICULTY_SET: StaticResourceLocation = rl!("minecraft:serversettings/difficulty/set");
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST: StaticResourceLocation = rl!("minecraft:serversettings/enforce_allowlist");
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST_SET: StaticResourceLocation = rl!("minecraft:serversettings/enforce_allowlist/set");
pub const SERVERSETTINGS_USE_ALLOWLIST: StaticResourceLocation = rl!("minecraft:serversettings/use_allowlist");
pub const SERVERSETTINGS_USE_ALLOWLIST_SET: StaticResourceLocation = rl!("minecraft:serversettings/use_allowlist/set");
pub const SERVERSETTINGS_MAX_PLAYERS: StaticResourceLocation = rl!("minecraft:serversettings/max_players");
pub const SERVERSETTINGS_MAX_PLAYERS_SET: StaticResourceLocation = rl!("minecraft:serversettings/max_players/set");
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS: StaticResourceLocation = rl!("minecraft:serversettings/pause_when_empty_seconds");
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS_SET: StaticResourceLocation = rl!("minecraft:serversettings/pause_when_empty_seconds/set");
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT: StaticResourceLocation = rl!("minecraft:serversettings/player_idle_timeout");
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT_SET: StaticResourceLocation = rl!("minecraft:serversettings/player_idle_timeout/set");
pub const SERVERSETTINGS_ALLOW_FLIGHT: StaticResourceLocation = rl!("minecraft:serversettings/allow_flight");
pub const SERVERSETTINGS_ALLOW_FLIGHT_SET: StaticResourceLocation = rl!("minecraft:serversettings/allow_flight/set");
pub const SERVERSETTINGS_MOTD: StaticResourceLocation = rl!("minecraft:serversettings/motd");
pub const SERVERSETTINGS_MOTD_SET: StaticResourceLocation = rl!("minecraft:serversettings/motd/set");
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS: StaticResourceLocation = rl!("minecraft:serversettings/spawn_protection_radius");
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS_SET: StaticResourceLocation = rl!("minecraft:serversettings/spawn_protection_radius/set");
pub const SERVERSETTINGS_FORCE_GAME_MODE: StaticResourceLocation = rl!("minecraft:serversettings/force_game_mode");
pub const SERVERSETTINGS_FORCE_GAME_MODE_SET: StaticResourceLocation = rl!("minecraft:serversettings/force_game_mode/set");
pub const SERVERSETTINGS_GAME_MODE: StaticResourceLocation = rl!("minecraft:serversettings/game_mode");
pub const SERVERSETTINGS_GAME_MODE_SET: StaticResourceLocation = rl!("minecraft:serversettings/game_mode/set");
pub const SERVERSETTINGS_VIEW_DISTANCE: StaticResourceLocation = rl!("minecraft:serversettings/view_distance");
pub const SERVERSETTINGS_VIEW_DISTANCE_SET: StaticResourceLocation = rl!("minecraft:serversettings/view_distance/set");
pub const SERVERSETTINGS_SIMULATION_DISTANCE: StaticResourceLocation = rl!("minecraft:serversettings/simulation_distance");
pub const SERVERSETTINGS_SIMULATION_DISTANCE_SET: StaticResourceLocation = rl!("minecraft:serversettings/simulation_distance/set");
pub const SERVERSETTINGS_ACCEPT_TRANSFERS: StaticResourceLocation = rl!("minecraft:serversettings/accept_transfers");
pub const SERVERSETTINGS_ACCEPT_TRANSFERS_SET: StaticResourceLocation = rl!("minecraft:serversettings/accept_transfers/set");
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL: StaticResourceLocation = rl!("minecraft:serversettings/status_heartbeat_interval");
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL_SET: StaticResourceLocation = rl!("minecraft:serversettings/status_heartbeat_interval/set");
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL: StaticResourceLocation = rl!("minecraft:serversettings/operator_user_permission_level");
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL_SET: StaticResourceLocation = rl!("minecraft:serversettings/operator_user_permission_level/set");
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS: StaticResourceLocation = rl!("minecraft:serversettings/hide_online_players");
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS_SET: StaticResourceLocation = rl!("minecraft:serversettings/hide_online_players/set");
pub const SERVERSETTINGS_STATUS_REPLIES: StaticResourceLocation = rl!("minecraft:serversettings/status_replies");
pub const SERVERSETTINGS_STATUS_REPLIES_SET: StaticResourceLocation = rl!("minecraft:serversettings/status_replies/set");
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE: StaticResourceLocation = rl!("minecraft:serversettings/entity_broadcast_range");
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE_SET: StaticResourceLocation = rl!("minecraft:serversettings/entity_broadcast_range/set");
pub const GAMERULES: StaticResourceLocation = rl!("minecraft:gamerules");
pub const GAMERULES_UPDATE: StaticResourceLocation = rl!("minecraft:gamerules/update");
pub const RPC_DISCOVER: StaticResourceLocation = rl!("minecraft:rpc.discover");

pub const ENTRIES: &[StaticResourceLocation] = &[
    ALLOWLIST,
    ALLOWLIST_SET,
    ALLOWLIST_ADD,
    ALLOWLIST_REMOVE,
    ALLOWLIST_CLEAR,
    BANS,
    BANS_SET,
    BANS_ADD,
    BANS_REMOVE,
    BANS_CLEAR,
    IP_BANS,
    IP_BANS_SET,
    IP_BANS_ADD,
    IP_BANS_REMOVE,
    IP_BANS_CLEAR,
    PLAYERS,
    PLAYERS_KICK,
    OPERATORS,
    OPERATORS_SET,
    OPERATORS_ADD,
    OPERATORS_REMOVE,
    OPERATORS_CLEAR,
    SERVER_STATUS,
    SERVER_SAVE,
    SERVER_STOP,
    SERVER_SYSTEM_MESSAGE,
    SERVERSETTINGS_AUTOSAVE,
    SERVERSETTINGS_AUTOSAVE_SET,
    SERVERSETTINGS_DIFFICULTY,
    SERVERSETTINGS_DIFFICULTY_SET,
    SERVERSETTINGS_ENFORCE_ALLOWLIST,
    SERVERSETTINGS_ENFORCE_ALLOWLIST_SET,
    SERVERSETTINGS_USE_ALLOWLIST,
    SERVERSETTINGS_USE_ALLOWLIST_SET,
    SERVERSETTINGS_MAX_PLAYERS,
    SERVERSETTINGS_MAX_PLAYERS_SET,
    SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS,
    SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS_SET,
    SERVERSETTINGS_PLAYER_IDLE_TIMEOUT,
    SERVERSETTINGS_PLAYER_IDLE_TIMEOUT_SET,
    SERVERSETTINGS_ALLOW_FLIGHT,
    SERVERSETTINGS_ALLOW_FLIGHT_SET,
    SERVERSETTINGS_MOTD,
    SERVERSETTINGS_MOTD_SET,
    SERVERSETTINGS_SPAWN_PROTECTION_RADIUS,
    SERVERSETTINGS_SPAWN_PROTECTION_RADIUS_SET,
    SERVERSETTINGS_FORCE_GAME_MODE,
    SERVERSETTINGS_FORCE_GAME_MODE_SET,
    SERVERSETTINGS_GAME_MODE,
    SERVERSETTINGS_GAME_MODE_SET,
    SERVERSETTINGS_VIEW_DISTANCE,
    SERVERSETTINGS_VIEW_DISTANCE_SET,
    SERVERSETTINGS_SIMULATION_DISTANCE,
    SERVERSETTINGS_SIMULATION_DISTANCE_SET,
    SERVERSETTINGS_ACCEPT_TRANSFERS,
    SERVERSETTINGS_ACCEPT_TRANSFERS_SET,
    SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL,
    SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL_SET,
    SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL,
    SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL_SET,
    SERVERSETTINGS_HIDE_ONLINE_PLAYERS,
    SERVERSETTINGS_HIDE_ONLINE_PLAYERS_SET,
    SERVERSETTINGS_STATUS_REPLIES,
    SERVERSETTINGS_STATUS_REPLIES_SET,
    SERVERSETTINGS_ENTITY_BROADCAST_RANGE,
    SERVERSETTINGS_ENTITY_BROADCAST_RANGE_SET,
    GAMERULES,
    GAMERULES_UPDATE,
    RPC_DISCOVER,
];
