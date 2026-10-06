// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum BlockPredicateType;
    MatchingBlocks = "minecraft:matching_blocks",
    MatchingBlockTag = "minecraft:matching_block_tag",
    MatchingFluids = "minecraft:matching_fluids",
    MatchingBiomes = "minecraft:matching_biomes",
    HasSturdyFace = "minecraft:has_sturdy_face",
    Solid = "minecraft:solid",
    Replaceable = "minecraft:replaceable",
    WouldSurvive = "minecraft:would_survive",
    InsideWorldBounds = "minecraft:inside_world_bounds",
    AnyOf = "minecraft:any_of",
    AllOf = "minecraft:all_of",
    Not = "minecraft:not",
    True = "minecraft:true",
    Unobstructed = "minecraft:unobstructed",
    HeightRange = "minecraft:height_range",
    VolumeMatch = "minecraft:volume_match",
    BelowHeightmap = "minecraft:below_heightmap",
}
