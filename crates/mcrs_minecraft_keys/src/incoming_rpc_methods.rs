// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALLOWLIST: Id<crate::IncomingRpcMethods> = Id::from_static(0);
pub const ALLOWLIST_ADD: Id<crate::IncomingRpcMethods> = Id::from_static(2);
pub const ALLOWLIST_CLEAR: Id<crate::IncomingRpcMethods> = Id::from_static(4);
pub const ALLOWLIST_REMOVE: Id<crate::IncomingRpcMethods> = Id::from_static(3);
pub const ALLOWLIST_SET: Id<crate::IncomingRpcMethods> = Id::from_static(1);
pub const BANS: Id<crate::IncomingRpcMethods> = Id::from_static(5);
pub const BANS_ADD: Id<crate::IncomingRpcMethods> = Id::from_static(7);
pub const BANS_CLEAR: Id<crate::IncomingRpcMethods> = Id::from_static(9);
pub const BANS_REMOVE: Id<crate::IncomingRpcMethods> = Id::from_static(8);
pub const BANS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(6);
pub const GAMERULES: Id<crate::IncomingRpcMethods> = Id::from_static(66);
pub const GAMERULES_UPDATE: Id<crate::IncomingRpcMethods> = Id::from_static(67);
pub const IP_BANS: Id<crate::IncomingRpcMethods> = Id::from_static(10);
pub const IP_BANS_ADD: Id<crate::IncomingRpcMethods> = Id::from_static(12);
pub const IP_BANS_CLEAR: Id<crate::IncomingRpcMethods> = Id::from_static(14);
pub const IP_BANS_REMOVE: Id<crate::IncomingRpcMethods> = Id::from_static(13);
pub const IP_BANS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(11);
pub const OPERATORS: Id<crate::IncomingRpcMethods> = Id::from_static(17);
pub const OPERATORS_ADD: Id<crate::IncomingRpcMethods> = Id::from_static(19);
pub const OPERATORS_CLEAR: Id<crate::IncomingRpcMethods> = Id::from_static(21);
pub const OPERATORS_REMOVE: Id<crate::IncomingRpcMethods> = Id::from_static(20);
pub const OPERATORS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(18);
pub const PLAYERS: Id<crate::IncomingRpcMethods> = Id::from_static(15);
pub const PLAYERS_KICK: Id<crate::IncomingRpcMethods> = Id::from_static(16);
pub const RPC_DISCOVER: Id<crate::IncomingRpcMethods> = Id::from_static(68);
pub const SERVER_SAVE: Id<crate::IncomingRpcMethods> = Id::from_static(23);
pub const SERVER_STATUS: Id<crate::IncomingRpcMethods> = Id::from_static(22);
pub const SERVER_STOP: Id<crate::IncomingRpcMethods> = Id::from_static(24);
pub const SERVER_SYSTEM_MESSAGE: Id<crate::IncomingRpcMethods> = Id::from_static(25);
pub const SERVERSETTINGS_ACCEPT_TRANSFERS: Id<crate::IncomingRpcMethods> = Id::from_static(54);
pub const SERVERSETTINGS_ACCEPT_TRANSFERS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(55);
pub const SERVERSETTINGS_ALLOW_FLIGHT: Id<crate::IncomingRpcMethods> = Id::from_static(40);
pub const SERVERSETTINGS_ALLOW_FLIGHT_SET: Id<crate::IncomingRpcMethods> = Id::from_static(41);
pub const SERVERSETTINGS_AUTOSAVE: Id<crate::IncomingRpcMethods> = Id::from_static(26);
pub const SERVERSETTINGS_AUTOSAVE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(27);
pub const SERVERSETTINGS_DIFFICULTY: Id<crate::IncomingRpcMethods> = Id::from_static(28);
pub const SERVERSETTINGS_DIFFICULTY_SET: Id<crate::IncomingRpcMethods> = Id::from_static(29);
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST: Id<crate::IncomingRpcMethods> = Id::from_static(30);
pub const SERVERSETTINGS_ENFORCE_ALLOWLIST_SET: Id<crate::IncomingRpcMethods> = Id::from_static(31);
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE: Id<crate::IncomingRpcMethods> = Id::from_static(64);
pub const SERVERSETTINGS_ENTITY_BROADCAST_RANGE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(65);
pub const SERVERSETTINGS_FORCE_GAME_MODE: Id<crate::IncomingRpcMethods> = Id::from_static(46);
pub const SERVERSETTINGS_FORCE_GAME_MODE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(47);
pub const SERVERSETTINGS_GAME_MODE: Id<crate::IncomingRpcMethods> = Id::from_static(48);
pub const SERVERSETTINGS_GAME_MODE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(49);
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS: Id<crate::IncomingRpcMethods> = Id::from_static(60);
pub const SERVERSETTINGS_HIDE_ONLINE_PLAYERS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(61);
pub const SERVERSETTINGS_MAX_PLAYERS: Id<crate::IncomingRpcMethods> = Id::from_static(34);
pub const SERVERSETTINGS_MAX_PLAYERS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(35);
pub const SERVERSETTINGS_MOTD: Id<crate::IncomingRpcMethods> = Id::from_static(42);
pub const SERVERSETTINGS_MOTD_SET: Id<crate::IncomingRpcMethods> = Id::from_static(43);
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL: Id<crate::IncomingRpcMethods> = Id::from_static(58);
pub const SERVERSETTINGS_OPERATOR_USER_PERMISSION_LEVEL_SET: Id<crate::IncomingRpcMethods> = Id::from_static(59);
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS: Id<crate::IncomingRpcMethods> = Id::from_static(36);
pub const SERVERSETTINGS_PAUSE_WHEN_EMPTY_SECONDS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(37);
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT: Id<crate::IncomingRpcMethods> = Id::from_static(38);
pub const SERVERSETTINGS_PLAYER_IDLE_TIMEOUT_SET: Id<crate::IncomingRpcMethods> = Id::from_static(39);
pub const SERVERSETTINGS_SIMULATION_DISTANCE: Id<crate::IncomingRpcMethods> = Id::from_static(52);
pub const SERVERSETTINGS_SIMULATION_DISTANCE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(53);
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS: Id<crate::IncomingRpcMethods> = Id::from_static(44);
pub const SERVERSETTINGS_SPAWN_PROTECTION_RADIUS_SET: Id<crate::IncomingRpcMethods> = Id::from_static(45);
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL: Id<crate::IncomingRpcMethods> = Id::from_static(56);
pub const SERVERSETTINGS_STATUS_HEARTBEAT_INTERVAL_SET: Id<crate::IncomingRpcMethods> = Id::from_static(57);
pub const SERVERSETTINGS_STATUS_REPLIES: Id<crate::IncomingRpcMethods> = Id::from_static(62);
pub const SERVERSETTINGS_STATUS_REPLIES_SET: Id<crate::IncomingRpcMethods> = Id::from_static(63);
pub const SERVERSETTINGS_USE_ALLOWLIST: Id<crate::IncomingRpcMethods> = Id::from_static(32);
pub const SERVERSETTINGS_USE_ALLOWLIST_SET: Id<crate::IncomingRpcMethods> = Id::from_static(33);
pub const SERVERSETTINGS_VIEW_DISTANCE: Id<crate::IncomingRpcMethods> = Id::from_static(50);
pub const SERVERSETTINGS_VIEW_DISTANCE_SET: Id<crate::IncomingRpcMethods> = Id::from_static(51);

pub const NAMES: &[&str] = &[
    "minecraft:allowlist",
    "minecraft:allowlist/set",
    "minecraft:allowlist/add",
    "minecraft:allowlist/remove",
    "minecraft:allowlist/clear",
    "minecraft:bans",
    "minecraft:bans/set",
    "minecraft:bans/add",
    "minecraft:bans/remove",
    "minecraft:bans/clear",
    "minecraft:ip_bans",
    "minecraft:ip_bans/set",
    "minecraft:ip_bans/add",
    "minecraft:ip_bans/remove",
    "minecraft:ip_bans/clear",
    "minecraft:players",
    "minecraft:players/kick",
    "minecraft:operators",
    "minecraft:operators/set",
    "minecraft:operators/add",
    "minecraft:operators/remove",
    "minecraft:operators/clear",
    "minecraft:server/status",
    "minecraft:server/save",
    "minecraft:server/stop",
    "minecraft:server/system_message",
    "minecraft:serversettings/autosave",
    "minecraft:serversettings/autosave/set",
    "minecraft:serversettings/difficulty",
    "minecraft:serversettings/difficulty/set",
    "minecraft:serversettings/enforce_allowlist",
    "minecraft:serversettings/enforce_allowlist/set",
    "minecraft:serversettings/use_allowlist",
    "minecraft:serversettings/use_allowlist/set",
    "minecraft:serversettings/max_players",
    "minecraft:serversettings/max_players/set",
    "minecraft:serversettings/pause_when_empty_seconds",
    "minecraft:serversettings/pause_when_empty_seconds/set",
    "minecraft:serversettings/player_idle_timeout",
    "minecraft:serversettings/player_idle_timeout/set",
    "minecraft:serversettings/allow_flight",
    "minecraft:serversettings/allow_flight/set",
    "minecraft:serversettings/motd",
    "minecraft:serversettings/motd/set",
    "minecraft:serversettings/spawn_protection_radius",
    "minecraft:serversettings/spawn_protection_radius/set",
    "minecraft:serversettings/force_game_mode",
    "minecraft:serversettings/force_game_mode/set",
    "minecraft:serversettings/game_mode",
    "minecraft:serversettings/game_mode/set",
    "minecraft:serversettings/view_distance",
    "minecraft:serversettings/view_distance/set",
    "minecraft:serversettings/simulation_distance",
    "minecraft:serversettings/simulation_distance/set",
    "minecraft:serversettings/accept_transfers",
    "minecraft:serversettings/accept_transfers/set",
    "minecraft:serversettings/status_heartbeat_interval",
    "minecraft:serversettings/status_heartbeat_interval/set",
    "minecraft:serversettings/operator_user_permission_level",
    "minecraft:serversettings/operator_user_permission_level/set",
    "minecraft:serversettings/hide_online_players",
    "minecraft:serversettings/hide_online_players/set",
    "minecraft:serversettings/status_replies",
    "minecraft:serversettings/status_replies/set",
    "minecraft:serversettings/entity_broadcast_range",
    "minecraft:serversettings/entity_broadcast_range/set",
    "minecraft:gamerules",
    "minecraft:gamerules/update",
    "minecraft:rpc.discover",
];
