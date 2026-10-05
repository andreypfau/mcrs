// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALL_OF: Id<crate::LootConditionType> = Id::from_static(2);
pub const ANY_OF: Id<crate::LootConditionType> = Id::from_static(1);
pub const DAMAGE_SOURCE_PROPERTIES: Id<crate::LootConditionType> = Id::from_static(12);
pub const ENCHANTMENT_ACTIVE_CHECK: Id<crate::LootConditionType> = Id::from_static(18);
pub const ENTITY_PROPERTIES: Id<crate::LootConditionType> = Id::from_static(5);
pub const ENTITY_SCORES: Id<crate::LootConditionType> = Id::from_static(7);
pub const ENVIRONMENT_ATTRIBUTE_CHECK: Id<crate::LootConditionType> = Id::from_static(19);
pub const FLOAT_VALUE_CHECK: Id<crate::LootConditionType> = Id::from_static(17);
pub const INT_VALUE_CHECK: Id<crate::LootConditionType> = Id::from_static(16);
pub const INVERTED: Id<crate::LootConditionType> = Id::from_static(0);
pub const KILLED_BY_PLAYER: Id<crate::LootConditionType> = Id::from_static(6);
pub const LOCATION_CHECK: Id<crate::LootConditionType> = Id::from_static(13);
pub const MATCH_BLOCK: Id<crate::LootConditionType> = Id::from_static(8);
pub const MATCH_TOOL: Id<crate::LootConditionType> = Id::from_static(9);
pub const RANDOM_CHANCE: Id<crate::LootConditionType> = Id::from_static(3);
pub const RANDOM_CHANCE_WITH_ENCHANTED_BONUS: Id<crate::LootConditionType> = Id::from_static(4);
pub const SURVIVES_EXPLOSION: Id<crate::LootConditionType> = Id::from_static(11);
pub const TABLE_BONUS: Id<crate::LootConditionType> = Id::from_static(10);
pub const TIME_CHECK: Id<crate::LootConditionType> = Id::from_static(15);
pub const WEATHER_CHECK: Id<crate::LootConditionType> = Id::from_static(14);

pub const NAMES: &[&str] = &[
    "minecraft:inverted",
    "minecraft:any_of",
    "minecraft:all_of",
    "minecraft:random_chance",
    "minecraft:random_chance_with_enchanted_bonus",
    "minecraft:entity_properties",
    "minecraft:killed_by_player",
    "minecraft:entity_scores",
    "minecraft:match_block",
    "minecraft:match_tool",
    "minecraft:table_bonus",
    "minecraft:survives_explosion",
    "minecraft:damage_source_properties",
    "minecraft:location_check",
    "minecraft:weather_check",
    "minecraft:time_check",
    "minecraft:int_value_check",
    "minecraft:float_value_check",
    "minecraft:enchantment_active_check",
    "minecraft:environment_attribute_check",
];
