// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum IntProviderType;
    Constant = "minecraft:constant",
    Uniform = "minecraft:uniform",
    BiasedToBottom = "minecraft:biased_to_bottom",
    VeryBiasedToBottom = "minecraft:very_biased_to_bottom",
    Clamped = "minecraft:clamped",
    WeightedList = "minecraft:weighted_list",
    ClampedNormal = "minecraft:clamped_normal",
    Trapezoid = "minecraft:trapezoid",
}
