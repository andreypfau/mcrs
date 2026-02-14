use serde::Deserialize;
use mcrs_protocol::Ident;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "condition")]
pub enum LootConditionProto {
    #[serde(rename = "minecraft:match_tool")]
    MatchTool { predicate: ToolPredicateProto },
    #[serde(rename = "minecraft:survives_explosion")]
    SurvivesExplosion {},
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Box<LootConditionProto> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { terms: Vec<LootConditionProto> },
    #[serde(rename = "minecraft:all_of")]
    AllOf { terms: Vec<LootConditionProto> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolPredicateProto {
    #[serde(default)]
    pub predicates: Option<ToolPredicatesProto>,
    #[serde(default)]
    pub items: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolPredicatesProto {
    #[serde(rename = "minecraft:enchantments", default)]
    pub enchantments: Option<Vec<EnchantmentPredicateProto>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnchantmentPredicateProto {
    pub enchantments: Ident<String>,
    #[serde(default)]
    pub levels: Option<LevelRange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LevelRange {
    #[serde(default)]
    pub min: Option<u8>,
    #[serde(default)]
    pub max: Option<u8>,
}

// Resolved runtime types

#[derive(Debug, Clone)]
pub enum LootCondition {
    MatchToolEnchantment {
        enchantment_registry_index: u16,
        min_level: u8,
    },
    SurvivesExplosion,
    Inverted(Box<LootCondition>),
    AnyOf(Vec<LootCondition>),
    AllOf(Vec<LootCondition>),
    AlwaysTrue,
}
