// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum LootConditionType;
    Inverted = "minecraft:inverted",
    AnyOf = "minecraft:any_of",
    AllOf = "minecraft:all_of",
    RandomChance = "minecraft:random_chance",
    RandomChanceWithEnchantedBonus = "minecraft:random_chance_with_enchanted_bonus",
    EntityProperties = "minecraft:entity_properties",
    KilledByPlayer = "minecraft:killed_by_player",
    EntityScores = "minecraft:entity_scores",
    MatchBlock = "minecraft:match_block",
    MatchTool = "minecraft:match_tool",
    TableBonus = "minecraft:table_bonus",
    SurvivesExplosion = "minecraft:survives_explosion",
    DamageSourceProperties = "minecraft:damage_source_properties",
    LocationCheck = "minecraft:location_check",
    WeatherCheck = "minecraft:weather_check",
    TimeCheck = "minecraft:time_check",
    IntValueCheck = "minecraft:int_value_check",
    FloatValueCheck = "minecraft:float_value_check",
    EnchantmentActiveCheck = "minecraft:enchantment_active_check",
    EnvironmentAttributeCheck = "minecraft:environment_attribute_check",
}
