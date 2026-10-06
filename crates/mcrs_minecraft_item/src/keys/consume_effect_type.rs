// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum ConsumeEffectType;
    ApplyEffects = "minecraft:apply_effects",
    RemoveEffects = "minecraft:remove_effects",
    ClearAllEffects = "minecraft:clear_all_effects",
    TeleportRandomly = "minecraft:teleport_randomly",
    PlaySound = "minecraft:play_sound",
}
