// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum BlockStateProviderType;
    CopyProperties = "minecraft:copy_properties",
    DualNoise = "minecraft:dual_noise",
    Noise = "minecraft:noise",
    NoiseThreshold = "minecraft:noise_threshold",
    RandomBlock = "minecraft:random_block",
    RandomizedInt = "minecraft:randomized_int",
    Rotated = "minecraft:rotated",
    RuleBased = "minecraft:rule_based",
    Simple = "minecraft:simple",
    Weighted = "minecraft:weighted",
}
