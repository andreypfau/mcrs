// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const AIR_DRAG_MODIFIER: Id<crate::Attribute> = Id::from_static(0);
pub const ARMOR: Id<crate::Attribute> = Id::from_static(1);
pub const ARMOR_TOUGHNESS: Id<crate::Attribute> = Id::from_static(2);
pub const ATTACK_DAMAGE: Id<crate::Attribute> = Id::from_static(3);
pub const ATTACK_KNOCKBACK: Id<crate::Attribute> = Id::from_static(4);
pub const ATTACK_SPEED: Id<crate::Attribute> = Id::from_static(5);
pub const BELOW_NAME_DISTANCE: Id<crate::Attribute> = Id::from_static(6);
pub const BLOCK_BREAK_SPEED: Id<crate::Attribute> = Id::from_static(7);
pub const BLOCK_INTERACTION_RANGE: Id<crate::Attribute> = Id::from_static(8);
pub const BOUNCINESS: Id<crate::Attribute> = Id::from_static(9);
pub const BURNING_TIME: Id<crate::Attribute> = Id::from_static(10);
pub const CAMERA_DISTANCE: Id<crate::Attribute> = Id::from_static(11);
pub const ENTITY_INTERACTION_RANGE: Id<crate::Attribute> = Id::from_static(13);
pub const EXPLOSION_KNOCKBACK_RESISTANCE: Id<crate::Attribute> = Id::from_static(12);
pub const FALL_DAMAGE_MULTIPLIER: Id<crate::Attribute> = Id::from_static(14);
pub const FLYING_SPEED: Id<crate::Attribute> = Id::from_static(15);
pub const FOLLOW_RANGE: Id<crate::Attribute> = Id::from_static(16);
pub const FRICTION_MODIFIER: Id<crate::Attribute> = Id::from_static(17);
pub const GRAVITY: Id<crate::Attribute> = Id::from_static(18);
pub const JUMP_STRENGTH: Id<crate::Attribute> = Id::from_static(19);
pub const KNOCKBACK_RESISTANCE: Id<crate::Attribute> = Id::from_static(20);
pub const LUCK: Id<crate::Attribute> = Id::from_static(21);
pub const MAX_ABSORPTION: Id<crate::Attribute> = Id::from_static(22);
pub const MAX_HEALTH: Id<crate::Attribute> = Id::from_static(23);
pub const MINING_EFFICIENCY: Id<crate::Attribute> = Id::from_static(24);
pub const MOVEMENT_EFFICIENCY: Id<crate::Attribute> = Id::from_static(25);
pub const MOVEMENT_SPEED: Id<crate::Attribute> = Id::from_static(26);
pub const NAME_TAG_DISTANCE: Id<crate::Attribute> = Id::from_static(27);
pub const OXYGEN_BONUS: Id<crate::Attribute> = Id::from_static(28);
pub const SAFE_FALL_DISTANCE: Id<crate::Attribute> = Id::from_static(29);
pub const SCALE: Id<crate::Attribute> = Id::from_static(30);
pub const SNEAKING_SPEED: Id<crate::Attribute> = Id::from_static(31);
pub const SPAWN_REINFORCEMENTS: Id<crate::Attribute> = Id::from_static(32);
pub const STEP_HEIGHT: Id<crate::Attribute> = Id::from_static(33);
pub const SUBMERGED_MINING_SPEED: Id<crate::Attribute> = Id::from_static(34);
pub const SWEEPING_DAMAGE_RATIO: Id<crate::Attribute> = Id::from_static(35);
pub const TEMPT_RANGE: Id<crate::Attribute> = Id::from_static(36);
pub const WATER_MOVEMENT_EFFICIENCY: Id<crate::Attribute> = Id::from_static(37);
pub const WAYPOINT_RECEIVE_RANGE: Id<crate::Attribute> = Id::from_static(39);
pub const WAYPOINT_TRANSMIT_RANGE: Id<crate::Attribute> = Id::from_static(38);

pub const NAMES: &[&str] = &[
    "minecraft:air_drag_modifier",
    "minecraft:armor",
    "minecraft:armor_toughness",
    "minecraft:attack_damage",
    "minecraft:attack_knockback",
    "minecraft:attack_speed",
    "minecraft:below_name_distance",
    "minecraft:block_break_speed",
    "minecraft:block_interaction_range",
    "minecraft:bounciness",
    "minecraft:burning_time",
    "minecraft:camera_distance",
    "minecraft:explosion_knockback_resistance",
    "minecraft:entity_interaction_range",
    "minecraft:fall_damage_multiplier",
    "minecraft:flying_speed",
    "minecraft:follow_range",
    "minecraft:friction_modifier",
    "minecraft:gravity",
    "minecraft:jump_strength",
    "minecraft:knockback_resistance",
    "minecraft:luck",
    "minecraft:max_absorption",
    "minecraft:max_health",
    "minecraft:mining_efficiency",
    "minecraft:movement_efficiency",
    "minecraft:movement_speed",
    "minecraft:name_tag_distance",
    "minecraft:oxygen_bonus",
    "minecraft:safe_fall_distance",
    "minecraft:scale",
    "minecraft:sneaking_speed",
    "minecraft:spawn_reinforcements",
    "minecraft:step_height",
    "minecraft:submerged_mining_speed",
    "minecraft:sweeping_damage_ratio",
    "minecraft:tempt_range",
    "minecraft:water_movement_efficiency",
    "minecraft:waypoint_transmit_range",
    "minecraft:waypoint_receive_range",
];
