// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALL_OF: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(0);
pub const APPLY_EXHAUSTION: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(8);
pub const APPLY_IMPULSE: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(7);
pub const APPLY_MOB_EFFECT: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(1);
pub const ATTRIBUTE: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(2);
pub const CHANGE_ITEM_DAMAGE: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(3);
pub const DAMAGE_ENTITY: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(4);
pub const EXPLODE: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(5);
pub const IGNITE: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(6);
pub const PLAY_SOUND: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(9);
pub const REPLACE_BLOCK: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(10);
pub const REPLACE_DISK: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(11);
pub const RUN_FUNCTION: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(12);
pub const SET_BLOCK_PROPERTIES: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(13);
pub const SPAWN_PARTICLES: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(14);
pub const SUMMON_ENTITY: Id<crate::EnchantmentLocationBasedEffectType> = Id::from_static(15);

pub const NAMES: &[&str] = &[
    "minecraft:all_of",
    "minecraft:apply_mob_effect",
    "minecraft:attribute",
    "minecraft:change_item_damage",
    "minecraft:damage_entity",
    "minecraft:explode",
    "minecraft:ignite",
    "minecraft:apply_impulse",
    "minecraft:apply_exhaustion",
    "minecraft:play_sound",
    "minecraft:replace_block",
    "minecraft:replace_disk",
    "minecraft:run_function",
    "minecraft:set_block_properties",
    "minecraft:spawn_particles",
    "minecraft:summon_entity",
];
