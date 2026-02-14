use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "function")]
pub enum LootFunctionProto {
    #[serde(rename = "minecraft:set_count")]
    SetCount {
        count: serde_json::Value,
        #[serde(default)]
        add: bool,
    },
    #[serde(rename = "minecraft:explosion_decay")]
    ExplosionDecay {},
    #[serde(rename = "minecraft:apply_bonus")]
    ApplyBonus {
        enchantment: String,
        formula: String,
        #[serde(default)]
        parameters: Option<serde_json::Value>,
    },
    #[serde(other)]
    Unknown,
}
