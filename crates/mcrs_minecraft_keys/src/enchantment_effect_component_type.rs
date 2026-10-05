// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const AMMO_USE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(13);
pub const ARMOR_EFFECTIVENESS: Id<crate::EnchantmentEffectComponentType> = Id::from_static(5);
pub const ATTRIBUTES: Id<crate::EnchantmentEffectComponentType> = Id::from_static(24);
pub const BLOCK_EXPERIENCE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(21);
pub const CROSSBOW_CHARGE_TIME: Id<crate::EnchantmentEffectComponentType> = Id::from_static(25);
pub const CROSSBOW_CHARGING_SOUNDS: Id<crate::EnchantmentEffectComponentType> = Id::from_static(26);
pub const DAMAGE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(2);
pub const DAMAGE_IMMUNITY: Id<crate::EnchantmentEffectComponentType> = Id::from_static(1);
pub const DAMAGE_PROTECTION: Id<crate::EnchantmentEffectComponentType> = Id::from_static(0);
pub const EQUIPMENT_DROPS: Id<crate::EnchantmentEffectComponentType> = Id::from_static(10);
pub const FISHING_LUCK_BONUS: Id<crate::EnchantmentEffectComponentType> = Id::from_static(20);
pub const FISHING_TIME_REDUCTION: Id<crate::EnchantmentEffectComponentType> = Id::from_static(19);
pub const HIT_BLOCK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(8);
pub const ITEM_DAMAGE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(9);
pub const KNOCKBACK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(4);
pub const LOCATION_CHANGED: Id<crate::EnchantmentEffectComponentType> = Id::from_static(11);
pub const MOB_EXPERIENCE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(22);
pub const POST_ATTACK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(6);
pub const POST_PIERCING_ATTACK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(7);
pub const PREVENT_ARMOR_CHANGE: Id<crate::EnchantmentEffectComponentType> = Id::from_static(29);
pub const PREVENT_EQUIPMENT_DROP: Id<crate::EnchantmentEffectComponentType> = Id::from_static(28);
pub const PROJECTILE_COUNT: Id<crate::EnchantmentEffectComponentType> = Id::from_static(17);
pub const PROJECTILE_PIERCING: Id<crate::EnchantmentEffectComponentType> = Id::from_static(14);
pub const PROJECTILE_SPAWNED: Id<crate::EnchantmentEffectComponentType> = Id::from_static(15);
pub const PROJECTILE_SPREAD: Id<crate::EnchantmentEffectComponentType> = Id::from_static(16);
pub const REPAIR_WITH_XP: Id<crate::EnchantmentEffectComponentType> = Id::from_static(23);
pub const SMASH_DAMAGE_PER_FALLEN_BLOCK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(3);
pub const TICK: Id<crate::EnchantmentEffectComponentType> = Id::from_static(12);
pub const TRIDENT_RETURN_ACCELERATION: Id<crate::EnchantmentEffectComponentType> = Id::from_static(18);
pub const TRIDENT_SOUND: Id<crate::EnchantmentEffectComponentType> = Id::from_static(27);
pub const TRIDENT_SPIN_ATTACK_STRENGTH: Id<crate::EnchantmentEffectComponentType> = Id::from_static(30);

pub const NAMES: &[&str] = &[
    "minecraft:damage_protection",
    "minecraft:damage_immunity",
    "minecraft:damage",
    "minecraft:smash_damage_per_fallen_block",
    "minecraft:knockback",
    "minecraft:armor_effectiveness",
    "minecraft:post_attack",
    "minecraft:post_piercing_attack",
    "minecraft:hit_block",
    "minecraft:item_damage",
    "minecraft:equipment_drops",
    "minecraft:location_changed",
    "minecraft:tick",
    "minecraft:ammo_use",
    "minecraft:projectile_piercing",
    "minecraft:projectile_spawned",
    "minecraft:projectile_spread",
    "minecraft:projectile_count",
    "minecraft:trident_return_acceleration",
    "minecraft:fishing_time_reduction",
    "minecraft:fishing_luck_bonus",
    "minecraft:block_experience",
    "minecraft:mob_experience",
    "minecraft:repair_with_xp",
    "minecraft:attributes",
    "minecraft:crossbow_charge_time",
    "minecraft:crossbow_charging_sounds",
    "minecraft:trident_sound",
    "minecraft:prevent_equipment_drop",
    "minecraft:prevent_armor_change",
    "minecraft:trident_spin_attack_strength",
];
