// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum EnchantmentEntityEffectType;
    AllOf = "minecraft:all_of",
    ApplyMobEffect = "minecraft:apply_mob_effect",
    ChangeItemDamage = "minecraft:change_item_damage",
    DamageEntity = "minecraft:damage_entity",
    Explode = "minecraft:explode",
    Ignite = "minecraft:ignite",
    ApplyImpulse = "minecraft:apply_impulse",
    ApplyExhaustion = "minecraft:apply_exhaustion",
    PlaySound = "minecraft:play_sound",
    ReplaceBlock = "minecraft:replace_block",
    ReplaceDisk = "minecraft:replace_disk",
    RunFunction = "minecraft:run_function",
    SetBlockProperties = "minecraft:set_block_properties",
    SpawnParticles = "minecraft:spawn_particles",
    SummonEntity = "minecraft:summon_entity",
}
