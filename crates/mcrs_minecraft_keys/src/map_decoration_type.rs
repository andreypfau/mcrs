// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ABANDONED_CAMP: Id<crate::MapDecorationType> = Id::from_static(35);
pub const ANCIENT_CITY: Id<crate::MapDecorationType> = Id::from_static(36);
pub const BANNER_BLACK: Id<crate::MapDecorationType> = Id::from_static(25);
pub const BANNER_BLUE: Id<crate::MapDecorationType> = Id::from_static(21);
pub const BANNER_BROWN: Id<crate::MapDecorationType> = Id::from_static(22);
pub const BANNER_CYAN: Id<crate::MapDecorationType> = Id::from_static(19);
pub const BANNER_GRAY: Id<crate::MapDecorationType> = Id::from_static(17);
pub const BANNER_GREEN: Id<crate::MapDecorationType> = Id::from_static(23);
pub const BANNER_LIGHT_BLUE: Id<crate::MapDecorationType> = Id::from_static(13);
pub const BANNER_LIGHT_GRAY: Id<crate::MapDecorationType> = Id::from_static(18);
pub const BANNER_LIME: Id<crate::MapDecorationType> = Id::from_static(15);
pub const BANNER_MAGENTA: Id<crate::MapDecorationType> = Id::from_static(12);
pub const BANNER_ORANGE: Id<crate::MapDecorationType> = Id::from_static(11);
pub const BANNER_PINK: Id<crate::MapDecorationType> = Id::from_static(16);
pub const BANNER_PURPLE: Id<crate::MapDecorationType> = Id::from_static(20);
pub const BANNER_RED: Id<crate::MapDecorationType> = Id::from_static(24);
pub const BANNER_WHITE: Id<crate::MapDecorationType> = Id::from_static(10);
pub const BANNER_YELLOW: Id<crate::MapDecorationType> = Id::from_static(14);
pub const BLUE_MARKER: Id<crate::MapDecorationType> = Id::from_static(3);
pub const DESERT_PYRAMID: Id<crate::MapDecorationType> = Id::from_static(37);
pub const FRAME: Id<crate::MapDecorationType> = Id::from_static(1);
pub const JUNGLE_TEMPLE: Id<crate::MapDecorationType> = Id::from_static(32);
pub const MANSION: Id<crate::MapDecorationType> = Id::from_static(8);
pub const MINESHAFT: Id<crate::MapDecorationType> = Id::from_static(38);
pub const MONUMENT: Id<crate::MapDecorationType> = Id::from_static(9);
pub const OCEAN_RUIN_WARM: Id<crate::MapDecorationType> = Id::from_static(39);
pub const PLAYER: Id<crate::MapDecorationType> = Id::from_static(0);
pub const PLAYER_OFF_LIMITS: Id<crate::MapDecorationType> = Id::from_static(7);
pub const PLAYER_OFF_MAP: Id<crate::MapDecorationType> = Id::from_static(6);
pub const RED_MARKER: Id<crate::MapDecorationType> = Id::from_static(2);
pub const RED_X: Id<crate::MapDecorationType> = Id::from_static(26);
pub const SWAMP_HUT: Id<crate::MapDecorationType> = Id::from_static(33);
pub const TARGET_POINT: Id<crate::MapDecorationType> = Id::from_static(5);
pub const TARGET_X: Id<crate::MapDecorationType> = Id::from_static(4);
pub const TRIAL_CHAMBERS: Id<crate::MapDecorationType> = Id::from_static(34);
pub const VILLAGE_DESERT: Id<crate::MapDecorationType> = Id::from_static(27);
pub const VILLAGE_PLAINS: Id<crate::MapDecorationType> = Id::from_static(28);
pub const VILLAGE_SAVANNA: Id<crate::MapDecorationType> = Id::from_static(29);
pub const VILLAGE_SNOWY: Id<crate::MapDecorationType> = Id::from_static(30);
pub const VILLAGE_TAIGA: Id<crate::MapDecorationType> = Id::from_static(31);

pub const NAMES: &[&str] = &[
    "minecraft:player",
    "minecraft:frame",
    "minecraft:red_marker",
    "minecraft:blue_marker",
    "minecraft:target_x",
    "minecraft:target_point",
    "minecraft:player_off_map",
    "minecraft:player_off_limits",
    "minecraft:mansion",
    "minecraft:monument",
    "minecraft:banner_white",
    "minecraft:banner_orange",
    "minecraft:banner_magenta",
    "minecraft:banner_light_blue",
    "minecraft:banner_yellow",
    "minecraft:banner_lime",
    "minecraft:banner_pink",
    "minecraft:banner_gray",
    "minecraft:banner_light_gray",
    "minecraft:banner_cyan",
    "minecraft:banner_purple",
    "minecraft:banner_blue",
    "minecraft:banner_brown",
    "minecraft:banner_green",
    "minecraft:banner_red",
    "minecraft:banner_black",
    "minecraft:red_x",
    "minecraft:village_desert",
    "minecraft:village_plains",
    "minecraft:village_savanna",
    "minecraft:village_snowy",
    "minecraft:village_taiga",
    "minecraft:jungle_temple",
    "minecraft:swamp_hut",
    "minecraft:trial_chambers",
    "minecraft:abandoned_camp",
    "minecraft:ancient_city",
    "minecraft:desert_pyramid",
    "minecraft:mineshaft",
    "minecraft:ocean_ruin_warm",
];
