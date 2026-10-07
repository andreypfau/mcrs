// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum TestEnvironmentDefinitionType;
    AllOf = "minecraft:all_of",
    ClockTime = "minecraft:clock_time",
    Difficulty = "minecraft:difficulty",
    Function = "minecraft:function",
    GameRules = "minecraft:game_rules",
    TimelineAttributes = "minecraft:timeline_attributes",
    Weather = "minecraft:weather",
}
