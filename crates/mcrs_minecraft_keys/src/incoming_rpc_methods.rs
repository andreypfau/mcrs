// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const ALLOWLIST: ResourceLocation<'static> = rl!("minecraft:allowlist");
pub const ALLOWLIST_SET: ResourceLocation<'static> = rl!("minecraft:allowlist/set");
pub const ALLOWLIST_ADD: ResourceLocation<'static> = rl!("minecraft:allowlist/add");
pub const ALLOWLIST_REMOVE: ResourceLocation<'static> = rl!("minecraft:allowlist/remove");
pub const ALLOWLIST_CLEAR: ResourceLocation<'static> = rl!("minecraft:allowlist/clear");
pub const BANS: ResourceLocation<'static> = rl!("minecraft:bans");
pub const BANS_SET: ResourceLocation<'static> = rl!("minecraft:bans/set");
pub const BANS_ADD: ResourceLocation<'static> = rl!("minecraft:bans/add");
pub const BANS_REMOVE: ResourceLocation<'static> = rl!("minecraft:bans/remove");
pub const BANS_CLEAR: ResourceLocation<'static> = rl!("minecraft:bans/clear");
pub const IP_BANS: ResourceLocation<'static> = rl!("minecraft:ip_bans");
pub const IP_BANS_SET: ResourceLocation<'static> = rl!("minecraft:ip_bans/set");
pub const IP_BANS_ADD: ResourceLocation<'static> = rl!("minecraft:ip_bans/add");
pub const IP_BANS_REMOVE: ResourceLocation<'static> = rl!("minecraft:ip_bans/remove");
pub const IP_BANS_CLEAR: ResourceLocation<'static> = rl!("minecraft:ip_bans/clear");
pub const PLAYERS: ResourceLocation<'static> = rl!("minecraft:players");
pub const PLAYERS_KICK: ResourceLocation<'static> = rl!("minecraft:players/kick");
pub const OPERATORS: ResourceLocation<'static> = rl!("minecraft:operators");
pub const OPERATORS_SET: ResourceLocation<'static> = rl!("minecraft:operators/set");
pub const OPERATORS_ADD: ResourceLocation<'static> = rl!("minecraft:operators/add");
pub const OPERATORS_REMOVE: ResourceLocation<'static> = rl!("minecraft:operators/remove");
pub const OPERATORS_CLEAR: ResourceLocation<'static> = rl!("minecraft:operators/clear");
pub const SERVER_STATUS: ResourceLocation<'static> = rl!("minecraft:server/status");
pub const SERVER_SAVE: ResourceLocation<'static> = rl!("minecraft:server/save");
pub const SERVER_STOP: ResourceLocation<'static> = rl!("minecraft:server/stop");
pub const SERVER_SYSTEM_MESSAGE: ResourceLocation<'static> = rl!("minecraft:server/system_message");
pub const SERVERSETTINGS_AUTOSAVE: ResourceLocation<'static> = rl!("minecraft:serversettings/autosave");
pub const SERVERSETTINGS_AUTOSAVE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/autosave/set");
pub const SERVERSETTINGS_DIFFICULTY: ResourceLocation<'static> = rl!("minecraft:serversettings/difficulty");
pub const SERVERSETTINGS_DIFFICULTY_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/difficulty/set");
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST: ResourceLocation<'static> = rl!("minecraft:serversettings/enforce_allowlist");
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/enforce_allowlist/set");
pub const SERVERSETTINGS_USE_ALLOWLIST: ResourceLocation<'static> = rl!("minecraft:serversettings/use_allowlist");
pub const SERVERSETTINGS_USE_ALLOWLIST_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/use_allowlist/set");
pub const SERVERSETTINGS_MAX_PLAYERS: ResourceLocation<'static> = rl!("minecraft:serversettings/max_players");
pub const SERVERSETTINGS_MAX_PLAYERS_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/max_players/set");
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS: ResourceLocation<'static> = rl!("minecraft:serversettings/pause_when_empty_seconds");
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/pause_when_empty_seconds/set");
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT: ResourceLocation<'static> = rl!("minecraft:serversettings/player_idle_timeout");
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/player_idle_timeout/set");
pub const SERVERSETTINGS_ALLOW_FLIGHT: ResourceLocation<'static> = rl!("minecraft:serversettings/allow_flight");
pub const SERVERSETTINGS_ALLOW_FLIGHT_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/allow_flight/set");
pub const SERVERSETTINGS_MOTD: ResourceLocation<'static> = rl!("minecraft:serversettings/motd");
pub const SERVERSETTINGS_MOTD_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/motd/set");
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS: ResourceLocation<'static> = rl!("minecraft:serversettings/spawn_protection_radius");
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/spawn_protection_radius/set");
pub const SERVERSETTINGS_FORCE_GAME_MODE: ResourceLocation<'static> = rl!("minecraft:serversettings/force_game_mode");
pub const SERVERSETTINGS_FORCE_GAME_MODE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/force_game_mode/set");
pub const SERVERSETTINGS_GAME_MODE: ResourceLocation<'static> = rl!("minecraft:serversettings/game_mode");
pub const SERVERSETTINGS_GAME_MODE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/game_mode/set");
pub const SERVERSETTINGS_VIEW_DISTANCE: ResourceLocation<'static> = rl!("minecraft:serversettings/view_distance");
pub const SERVERSETTINGS_VIEW_DISTANCE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/view_distance/set");
pub const SERVERSETTINGS_SIMULATION_DISTANCE: ResourceLocation<'static> = rl!("minecraft:serversettings/simulation_distance");
pub const SERVERSETTINGS_SIMULATION_DISTANCE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/simulation_distance/set");
pub const SERVERSETTINGS_ACCEPT_TRANSFERS: ResourceLocation<'static> = rl!("minecraft:serversettings/accept_transfers");
pub const SERVERSETTINGS_ACCEPT_TRANSFERS_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/accept_transfers/set");
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL: ResourceLocation<'static> = rl!("minecraft:serversettings/status_heartbeat_interval");
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/status_heartbeat_interval/set");
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL: ResourceLocation<'static> = rl!("minecraft:serversettings/operator_user_permission_level");
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/operator_user_permission_level/set");
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS: ResourceLocation<'static> = rl!("minecraft:serversettings/hide_online_players");
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/hide_online_players/set");
pub const SERVERSETTINGS_STATUS_REPLIES: ResourceLocation<'static> = rl!("minecraft:serversettings/status_replies");
pub const SERVERSETTINGS_STATUS_REPLIES_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/status_replies/set");
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE: ResourceLocation<'static> = rl!("minecraft:serversettings/entity_broadcast_range");
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE_SET: ResourceLocation<'static> = rl!("minecraft:serversettings/entity_broadcast_range/set");
pub const GAMERULES: ResourceLocation<'static> = rl!("minecraft:gamerules");
pub const GAMERULES_UPDATE: ResourceLocation<'static> = rl!("minecraft:gamerules/update");
pub const RPC_DISCOVER: ResourceLocation<'static> = rl!("minecraft:rpc.discover");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
