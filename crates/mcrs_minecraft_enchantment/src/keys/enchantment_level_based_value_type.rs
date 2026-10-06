// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum EnchantmentLevelBasedValueType;
    Clamped = "minecraft:clamped",
    Fraction = "minecraft:fraction",
    LevelsSquared = "minecraft:levels_squared",
    Linear = "minecraft:linear",
    Exponent = "minecraft:exponent",
    Lookup = "minecraft:lookup",
}
