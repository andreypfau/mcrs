// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

mcrs_minecraft_registry::static_keys! {
    crate::LootConditionType;
    INVERTED = "minecraft:inverted",
    ANY_OF = "minecraft:any_of",
    ALL_OF = "minecraft:all_of",
    RANDOM_CHANCE = "minecraft:random_chance",
    RANDOM_CHANCE_WITH_ENCHANTED_BONUS = "minecraft:random_chance_with_enchanted_bonus",
    ENTITY_PROPERTIES = "minecraft:entity_properties",
    KILLED_BY_PLAYER = "minecraft:killed_by_player",
    ENTITY_SCORES = "minecraft:entity_scores",
    MATCH_BLOCK = "minecraft:match_block",
    MATCH_TOOL = "minecraft:match_tool",
    TABLE_BONUS = "minecraft:table_bonus",
    SURVIVES_EXPLOSION = "minecraft:survives_explosion",
    DAMAGE_SOURCE_PROPERTIES = "minecraft:damage_source_properties",
    LOCATION_CHECK = "minecraft:location_check",
    WEATHER_CHECK = "minecraft:weather_check",
    TIME_CHECK = "minecraft:time_check",
    INT_VALUE_CHECK = "minecraft:int_value_check",
    FLOAT_VALUE_CHECK = "minecraft:float_value_check",
    ENCHANTMENT_ACTIVE_CHECK = "minecraft:enchantment_active_check",
    ENVIRONMENT_ATTRIBUTE_CHECK = "minecraft:environment_attribute_check",
}
