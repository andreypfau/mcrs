// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ADVANCE_TIME: Id<crate::GameRule> = Id::from_static(0);
pub const ADVANCE_WEATHER: Id<crate::GameRule> = Id::from_static(1);
pub const ALLOW_ENTERING_NETHER_USING_PORTALS: Id<crate::GameRule> = Id::from_static(2);
pub const BLOCK_DROPS: Id<crate::GameRule> = Id::from_static(3);
pub const BLOCK_EXPLOSION_DROP_DECAY: Id<crate::GameRule> = Id::from_static(4);
pub const COMMAND_BLOCK_OUTPUT: Id<crate::GameRule> = Id::from_static(6);
pub const COMMAND_BLOCKS_WORK: Id<crate::GameRule> = Id::from_static(5);
pub const DROWNING_DAMAGE: Id<crate::GameRule> = Id::from_static(7);
pub const ELYTRA_MOVEMENT_CHECK: Id<crate::GameRule> = Id::from_static(8);
pub const ENDER_PEARLS_VANISH_ON_DEATH: Id<crate::GameRule> = Id::from_static(9);
pub const ENTITY_DROPS: Id<crate::GameRule> = Id::from_static(10);
pub const FALL_DAMAGE: Id<crate::GameRule> = Id::from_static(11);
pub const FIRE_DAMAGE: Id<crate::GameRule> = Id::from_static(12);
pub const FIRE_SPREAD_RADIUS_AROUND_PLAYER: Id<crate::GameRule> = Id::from_static(13);
pub const FORGIVE_DEAD_PLAYERS: Id<crate::GameRule> = Id::from_static(14);
pub const FREEZE_DAMAGE: Id<crate::GameRule> = Id::from_static(15);
pub const GLOBAL_SOUND_EVENTS: Id<crate::GameRule> = Id::from_static(16);
pub const IMMEDIATE_RESPAWN: Id<crate::GameRule> = Id::from_static(17);
pub const KEEP_INVENTORY: Id<crate::GameRule> = Id::from_static(18);
pub const LAVA_SOURCE_CONVERSION: Id<crate::GameRule> = Id::from_static(19);
pub const LIMITED_CRAFTING: Id<crate::GameRule> = Id::from_static(20);
pub const LOCATOR_BAR: Id<crate::GameRule> = Id::from_static(21);
pub const LOG_ADMIN_COMMANDS: Id<crate::GameRule> = Id::from_static(22);
pub const MAX_BLOCK_MODIFICATIONS: Id<crate::GameRule> = Id::from_static(23);
pub const MAX_COMMAND_FORKS: Id<crate::GameRule> = Id::from_static(24);
pub const MAX_COMMAND_SEQUENCE_LENGTH: Id<crate::GameRule> = Id::from_static(25);
pub const MAX_ENTITY_CRAMMING: Id<crate::GameRule> = Id::from_static(26);
pub const MAX_MINECART_SPEED: Id<crate::GameRule> = Id::from_static(27);
pub const MAX_SNOW_ACCUMULATION_HEIGHT: Id<crate::GameRule> = Id::from_static(28);
pub const MOB_DROPS: Id<crate::GameRule> = Id::from_static(29);
pub const MOB_EXPLOSION_DROP_DECAY: Id<crate::GameRule> = Id::from_static(30);
pub const MOB_GRIEFING: Id<crate::GameRule> = Id::from_static(31);
pub const NATURAL_HEALTH_REGENERATION: Id<crate::GameRule> = Id::from_static(32);
pub const PLAYER_MOVEMENT_CHECK: Id<crate::GameRule> = Id::from_static(33);
pub const PLAYERS_NETHER_PORTAL_CREATIVE_DELAY: Id<crate::GameRule> = Id::from_static(34);
pub const PLAYERS_NETHER_PORTAL_DEFAULT_DELAY: Id<crate::GameRule> = Id::from_static(35);
pub const PLAYERS_SLEEPING_PERCENTAGE: Id<crate::GameRule> = Id::from_static(36);
pub const PROJECTILES_CAN_BREAK_BLOCKS: Id<crate::GameRule> = Id::from_static(37);
pub const PVP: Id<crate::GameRule> = Id::from_static(38);
pub const RAIDS: Id<crate::GameRule> = Id::from_static(39);
pub const RANDOM_TICK_SPEED: Id<crate::GameRule> = Id::from_static(40);
pub const REDUCED_DEBUG_INFO: Id<crate::GameRule> = Id::from_static(41);
pub const RESPAWN_RADIUS: Id<crate::GameRule> = Id::from_static(42);
pub const SEND_COMMAND_FEEDBACK: Id<crate::GameRule> = Id::from_static(43);
pub const SHOW_ADVANCEMENT_MESSAGES: Id<crate::GameRule> = Id::from_static(44);
pub const SHOW_DEATH_MESSAGES: Id<crate::GameRule> = Id::from_static(45);
pub const SPAWN_MOBS: Id<crate::GameRule> = Id::from_static(47);
pub const SPAWN_MONSTERS: Id<crate::GameRule> = Id::from_static(48);
pub const SPAWN_PATROLS: Id<crate::GameRule> = Id::from_static(49);
pub const SPAWN_PHANTOMS: Id<crate::GameRule> = Id::from_static(50);
pub const SPAWN_WANDERING_TRADERS: Id<crate::GameRule> = Id::from_static(51);
pub const SPAWN_WARDENS: Id<crate::GameRule> = Id::from_static(52);
pub const SPAWNER_BLOCKS_WORK: Id<crate::GameRule> = Id::from_static(46);
pub const SPECTATORS_GENERATE_CHUNKS: Id<crate::GameRule> = Id::from_static(53);
pub const SPREAD_VINES: Id<crate::GameRule> = Id::from_static(54);
pub const TNT_EXPLODES: Id<crate::GameRule> = Id::from_static(55);
pub const TNT_EXPLOSION_DROP_DECAY: Id<crate::GameRule> = Id::from_static(56);
pub const UNIVERSAL_ANGER: Id<crate::GameRule> = Id::from_static(57);
pub const WATER_SOURCE_CONVERSION: Id<crate::GameRule> = Id::from_static(58);

pub const NAMES: &[&str] = &[
    "minecraft:advance_time",
    "minecraft:advance_weather",
    "minecraft:allow_entering_nether_using_portals",
    "minecraft:block_drops",
    "minecraft:block_explosion_drop_decay",
    "minecraft:command_blocks_work",
    "minecraft:command_block_output",
    "minecraft:drowning_damage",
    "minecraft:elytra_movement_check",
    "minecraft:ender_pearls_vanish_on_death",
    "minecraft:entity_drops",
    "minecraft:fall_damage",
    "minecraft:fire_damage",
    "minecraft:fire_spread_radius_around_player",
    "minecraft:forgive_dead_players",
    "minecraft:freeze_damage",
    "minecraft:global_sound_events",
    "minecraft:immediate_respawn",
    "minecraft:keep_inventory",
    "minecraft:lava_source_conversion",
    "minecraft:limited_crafting",
    "minecraft:locator_bar",
    "minecraft:log_admin_commands",
    "minecraft:max_block_modifications",
    "minecraft:max_command_forks",
    "minecraft:max_command_sequence_length",
    "minecraft:max_entity_cramming",
    "minecraft:max_minecart_speed",
    "minecraft:max_snow_accumulation_height",
    "minecraft:mob_drops",
    "minecraft:mob_explosion_drop_decay",
    "minecraft:mob_griefing",
    "minecraft:natural_health_regeneration",
    "minecraft:player_movement_check",
    "minecraft:players_nether_portal_creative_delay",
    "minecraft:players_nether_portal_default_delay",
    "minecraft:players_sleeping_percentage",
    "minecraft:projectiles_can_break_blocks",
    "minecraft:pvp",
    "minecraft:raids",
    "minecraft:random_tick_speed",
    "minecraft:reduced_debug_info",
    "minecraft:respawn_radius",
    "minecraft:send_command_feedback",
    "minecraft:show_advancement_messages",
    "minecraft:show_death_messages",
    "minecraft:spawner_blocks_work",
    "minecraft:spawn_mobs",
    "minecraft:spawn_monsters",
    "minecraft:spawn_patrols",
    "minecraft:spawn_phantoms",
    "minecraft:spawn_wandering_traders",
    "minecraft:spawn_wardens",
    "minecraft:spectators_generate_chunks",
    "minecraft:spread_vines",
    "minecraft:tnt_explodes",
    "minecraft:tnt_explosion_drop_decay",
    "minecraft:universal_anger",
    "minecraft:water_source_conversion",
];
