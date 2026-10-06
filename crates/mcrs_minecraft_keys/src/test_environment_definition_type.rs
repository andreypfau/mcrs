// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const ALL_OF: StaticResourceLocation = rl!("minecraft:all_of");
pub const CLOCK_TIME: StaticResourceLocation = rl!("minecraft:clock_time");
pub const DIFFICULTY: StaticResourceLocation = rl!("minecraft:difficulty");
pub const FUNCTION: StaticResourceLocation = rl!("minecraft:function");
pub const GAME_RULES: StaticResourceLocation = rl!("minecraft:game_rules");
pub const TIMELINE_ATTRIBUTES: StaticResourceLocation = rl!("minecraft:timeline_attributes");
pub const WEATHER: StaticResourceLocation = rl!("minecraft:weather");

pub const ENTRIES: &[StaticResourceLocation] = &[
    ALL_OF,
    CLOCK_TIME,
    DIFFICULTY,
    FUNCTION,
    GAME_RULES,
    TIMELINE_ATTRIBUTES,
    WEATHER,
];
