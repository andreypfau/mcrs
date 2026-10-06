// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum RuleTestType;
    AllOf = "minecraft:all_of",
    AlwaysTrue = "minecraft:always_true",
    AnyOf = "minecraft:any_of",
    BlockMatch = "minecraft:block_match",
    BlockstateMatch = "minecraft:blockstate_match",
    HeightMatch = "minecraft:height_match",
    Not = "minecraft:not",
    RandomBlockMatch = "minecraft:random_block_match",
    RandomBlockstateMatch = "minecraft:random_blockstate_match",
    TagMatch = "minecraft:tag_match",
}
