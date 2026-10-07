// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum EnchantmentProviderType;
    ByCost = "minecraft:by_cost",
    ByCostWithDifficulty = "minecraft:by_cost_with_difficulty",
    Single = "minecraft:single",
}
