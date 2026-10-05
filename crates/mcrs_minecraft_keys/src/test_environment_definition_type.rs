// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALL_OF: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(0);
pub const CLOCK_TIME: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(1);
pub const DIFFICULTY: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(2);
pub const FUNCTION: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(3);
pub const GAME_RULES: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(4);
pub const TIMELINE_ATTRIBUTES: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(5);
pub const WEATHER: Id<crate::TestEnvironmentDefinitionType> = Id::from_static(6);

pub const NAMES: &[&str] = &[
    "minecraft:all_of",
    "minecraft:clock_time",
    "minecraft:difficulty",
    "minecraft:function",
    "minecraft:game_rules",
    "minecraft:timeline_attributes",
    "minecraft:weather",
];
