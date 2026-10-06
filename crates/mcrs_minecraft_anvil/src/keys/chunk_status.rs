// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum ChunkStatus;
    Empty = "minecraft:empty",
    StructureStarts = "minecraft:structure_starts",
    StructureReferences = "minecraft:structure_references",
    NoiseBiomes = "minecraft:noise_biomes",
    Biomes = "minecraft:biomes",
    Terrain = "minecraft:terrain",
    Features = "minecraft:features",
    InitializeLight = "minecraft:initialize_light",
    Light = "minecraft:light",
    Spawn = "minecraft:spawn",
    Full = "minecraft:full",
}
