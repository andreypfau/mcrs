// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const APPLY_EFFECTS: Id<crate::ConsumeEffectType> = Id::from_static(0);
pub const CLEAR_ALL_EFFECTS: Id<crate::ConsumeEffectType> = Id::from_static(2);
pub const PLAY_SOUND: Id<crate::ConsumeEffectType> = Id::from_static(4);
pub const REMOVE_EFFECTS: Id<crate::ConsumeEffectType> = Id::from_static(1);
pub const TELEPORT_RANDOMLY: Id<crate::ConsumeEffectType> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:apply_effects",
    "minecraft:remove_effects",
    "minecraft:clear_all_effects",
    "minecraft:teleport_randomly",
    "minecraft:play_sound",
];
