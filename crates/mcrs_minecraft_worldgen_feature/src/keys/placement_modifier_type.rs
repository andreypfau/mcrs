// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum PlacementModifierType;
    BlockPredicateFilter = "minecraft:block_predicate_filter",
    RarityFilter = "minecraft:rarity_filter",
    RandomChance = "minecraft:random_chance",
    SurfaceRelativeThresholdFilter = "minecraft:surface_relative_threshold_filter",
    SurfaceWaterDepthFilter = "minecraft:surface_water_depth_filter",
    Biome = "minecraft:biome",
    Count = "minecraft:count",
    NoiseBasedCount = "minecraft:noise_based_count",
    NoiseThresholdCount = "minecraft:noise_threshold_count",
    CountOnEveryLayer = "minecraft:count_on_every_layer",
    Cuboid = "minecraft:cuboid",
    EnvironmentScan = "minecraft:environment_scan",
    Heightmap = "minecraft:heightmap",
    HeightRange = "minecraft:height_range",
    InSquare = "minecraft:in_square",
    Offset = "minecraft:offset",
    RandomlySelected = "minecraft:randomly_selected",
    FixedPlacement = "minecraft:fixed_placement",
}
