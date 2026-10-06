// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum FloatProviderType;
    Constant = "minecraft:constant",
    Uniform = "minecraft:uniform",
    ClampedNormal = "minecraft:clamped_normal",
    Trapezoid = "minecraft:trapezoid",
}
