// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const AUDIO_AMBIENT_SOUNDS: Id<crate::EnvironmentAttribute> = Id::from_static(27);
pub const AUDIO_BACKGROUND_MUSIC: Id<crate::EnvironmentAttribute> = Id::from_static(25);
pub const AUDIO_FIREFLY_BUSH_SOUNDS: Id<crate::EnvironmentAttribute> = Id::from_static(28);
pub const AUDIO_MUSIC_VOLUME: Id<crate::EnvironmentAttribute> = Id::from_static(26);
pub const GAMEPLAY_BABY_VILLAGER_ACTIVITY: Id<crate::EnvironmentAttribute> = Id::from_static(51);
pub const GAMEPLAY_BED_RULE: Id<crate::EnvironmentAttribute> = Id::from_static(32);
pub const GAMEPLAY_BEES_STAY_IN_HIVE: Id<crate::EnvironmentAttribute> = Id::from_static(45);
pub const GAMEPLAY_CAN_PILLAGER_PATROL_SPAWN: Id<crate::EnvironmentAttribute> = Id::from_static(47);
pub const GAMEPLAY_CAN_START_RAID: Id<crate::EnvironmentAttribute> = Id::from_static(30);
pub const GAMEPLAY_CAT_WAKING_UP_GIFT_CHANCE: Id<crate::EnvironmentAttribute> = Id::from_static(44);
pub const GAMEPLAY_CREAKING_ACTIVE: Id<crate::EnvironmentAttribute> = Id::from_static(42);
pub const GAMEPLAY_CREATURE_WORLD_GEN_SPAWN_PROBABILITY: Id<crate::EnvironmentAttribute> = Id::from_static(49);
pub const GAMEPLAY_EYEBLOSSOM_OPEN: Id<crate::EnvironmentAttribute> = Id::from_static(38);
pub const GAMEPLAY_FAST_LAVA: Id<crate::EnvironmentAttribute> = Id::from_static(36);
pub const GAMEPLAY_INCREASED_FIRE_BURNOUT: Id<crate::EnvironmentAttribute> = Id::from_static(37);
pub const GAMEPLAY_MONSTERS_BURN: Id<crate::EnvironmentAttribute> = Id::from_static(46);
pub const GAMEPLAY_NATURAL_MOB_SPAWNS: Id<crate::EnvironmentAttribute> = Id::from_static(48);
pub const GAMEPLAY_NETHER_PORTAL_SPAWNS_PIGLIN: Id<crate::EnvironmentAttribute> = Id::from_static(35);
pub const GAMEPLAY_PIGLINS_ZOMBIFY: Id<crate::EnvironmentAttribute> = Id::from_static(40);
pub const GAMEPLAY_RESPAWN_ANCHOR_WORKS: Id<crate::EnvironmentAttribute> = Id::from_static(34);
pub const GAMEPLAY_SKY_LIGHT_LEVEL: Id<crate::EnvironmentAttribute> = Id::from_static(29);
pub const GAMEPLAY_SNOW_GOLEM_MELTS: Id<crate::EnvironmentAttribute> = Id::from_static(41);
pub const GAMEPLAY_STRAW_BED_RULE: Id<crate::EnvironmentAttribute> = Id::from_static(33);
pub const GAMEPLAY_SURFACE_SLIME_SPAWN_CHANCE: Id<crate::EnvironmentAttribute> = Id::from_static(43);
pub const GAMEPLAY_TURTLE_EGG_HATCH_CHANCE: Id<crate::EnvironmentAttribute> = Id::from_static(39);
pub const GAMEPLAY_VILLAGER_ACTIVITY: Id<crate::EnvironmentAttribute> = Id::from_static(50);
pub const GAMEPLAY_WATER_EVAPORATES: Id<crate::EnvironmentAttribute> = Id::from_static(31);
pub const VISUAL_AMBIENT_LIGHT_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(22);
pub const VISUAL_AMBIENT_PARTICLES: Id<crate::EnvironmentAttribute> = Id::from_static(24);
pub const VISUAL_BLOCK_LIGHT_TINT: Id<crate::EnvironmentAttribute> = Id::from_static(18);
pub const VISUAL_CLOUD_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(10);
pub const VISUAL_CLOUD_FOG_END_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(4);
pub const VISUAL_CLOUD_HEIGHT: Id<crate::EnvironmentAttribute> = Id::from_static(11);
pub const VISUAL_DEFAULT_DRIPSTONE_PARTICLE: Id<crate::EnvironmentAttribute> = Id::from_static(23);
pub const VISUAL_FOG_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(0);
pub const VISUAL_FOG_END_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(2);
pub const VISUAL_FOG_START_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(1);
pub const VISUAL_HAS_SKY_OCCLUDER: Id<crate::EnvironmentAttribute> = Id::from_static(17);
pub const VISUAL_MOON_ANGLE: Id<crate::EnvironmentAttribute> = Id::from_static(13);
pub const VISUAL_MOON_PHASE: Id<crate::EnvironmentAttribute> = Id::from_static(15);
pub const VISUAL_NIGHT_VISION_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(21);
pub const VISUAL_SKY_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(8);
pub const VISUAL_SKY_FOG_END_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(3);
pub const VISUAL_SKY_LIGHT_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(19);
pub const VISUAL_SKY_LIGHT_FACTOR: Id<crate::EnvironmentAttribute> = Id::from_static(20);
pub const VISUAL_STAR_ANGLE: Id<crate::EnvironmentAttribute> = Id::from_static(14);
pub const VISUAL_STAR_BRIGHTNESS: Id<crate::EnvironmentAttribute> = Id::from_static(16);
pub const VISUAL_SUN_ANGLE: Id<crate::EnvironmentAttribute> = Id::from_static(12);
pub const VISUAL_SUNRISE_SUNSET_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(9);
pub const VISUAL_WATER_FOG_COLOR: Id<crate::EnvironmentAttribute> = Id::from_static(5);
pub const VISUAL_WATER_FOG_END_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(7);
pub const VISUAL_WATER_FOG_START_DISTANCE: Id<crate::EnvironmentAttribute> = Id::from_static(6);

pub const NAMES: &[&str] = &[
    "minecraft:visual/fog_color",
    "minecraft:visual/fog_start_distance",
    "minecraft:visual/fog_end_distance",
    "minecraft:visual/sky_fog_end_distance",
    "minecraft:visual/cloud_fog_end_distance",
    "minecraft:visual/water_fog_color",
    "minecraft:visual/water_fog_start_distance",
    "minecraft:visual/water_fog_end_distance",
    "minecraft:visual/sky_color",
    "minecraft:visual/sunrise_sunset_color",
    "minecraft:visual/cloud_color",
    "minecraft:visual/cloud_height",
    "minecraft:visual/sun_angle",
    "minecraft:visual/moon_angle",
    "minecraft:visual/star_angle",
    "minecraft:visual/moon_phase",
    "minecraft:visual/star_brightness",
    "minecraft:visual/has_sky_occluder",
    "minecraft:visual/block_light_tint",
    "minecraft:visual/sky_light_color",
    "minecraft:visual/sky_light_factor",
    "minecraft:visual/night_vision_color",
    "minecraft:visual/ambient_light_color",
    "minecraft:visual/default_dripstone_particle",
    "minecraft:visual/ambient_particles",
    "minecraft:audio/background_music",
    "minecraft:audio/music_volume",
    "minecraft:audio/ambient_sounds",
    "minecraft:audio/firefly_bush_sounds",
    "minecraft:gameplay/sky_light_level",
    "minecraft:gameplay/can_start_raid",
    "minecraft:gameplay/water_evaporates",
    "minecraft:gameplay/bed_rule",
    "minecraft:gameplay/straw_bed_rule",
    "minecraft:gameplay/respawn_anchor_works",
    "minecraft:gameplay/nether_portal_spawns_piglin",
    "minecraft:gameplay/fast_lava",
    "minecraft:gameplay/increased_fire_burnout",
    "minecraft:gameplay/eyeblossom_open",
    "minecraft:gameplay/turtle_egg_hatch_chance",
    "minecraft:gameplay/piglins_zombify",
    "minecraft:gameplay/snow_golem_melts",
    "minecraft:gameplay/creaking_active",
    "minecraft:gameplay/surface_slime_spawn_chance",
    "minecraft:gameplay/cat_waking_up_gift_chance",
    "minecraft:gameplay/bees_stay_in_hive",
    "minecraft:gameplay/monsters_burn",
    "minecraft:gameplay/can_pillager_patrol_spawn",
    "minecraft:gameplay/natural_mob_spawns",
    "minecraft:gameplay/creature_world_gen_spawn_probability",
    "minecraft:gameplay/villager_activity",
    "minecraft:gameplay/baby_villager_activity",
];
