// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum RuleBlockEntityModifier;
    Clear = "minecraft:clear",
    Passthrough = "minecraft:passthrough",
    AppendStatic = "minecraft:append_static",
    AppendLoot = "minecraft:append_loot",
}
