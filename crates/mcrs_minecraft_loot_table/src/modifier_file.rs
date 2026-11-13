use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::condition_file::ConditionFile;
use crate::number_file::NumberFile;

/// An item modifier as a data pack writes it: a list applied in order, the key of a registered
/// modifier, or one object.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ModifierFile {
    Many(Vec<ModifierObject>),
    Key(ResourceLocationBuf),
    One(Box<ModifierObject>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModifierObject {
    pub condition: Option<ConditionFile>,
    #[serde(flatten)]
    pub kind: ModifierKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ModifierKind {
    #[serde(rename = "minecraft:set_count", alias = "set_count")]
    SetCount {
        count: NumberFile,
        #[serde(default)]
        add: bool,
    },
    #[serde(rename = "minecraft:explosion_decay", alias = "explosion_decay")]
    ExplosionDecay,
    #[serde(rename = "minecraft:limit_count", alias = "limit_count")]
    LimitCount { limit: LimitFile },
    #[serde(rename = "minecraft:apply_bonus", alias = "apply_bonus")]
    ApplyBonus {
        enchantment: ResourceLocationBuf,
        formula: ResourceLocationBuf,
        #[serde(default)]
        parameters: BonusParameters,
    },
    #[serde(rename = "minecraft:sequence", alias = "sequence")]
    Sequence { functions: Vec<ModifierObject> },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LimitFile {
    Exact(i32),
    Range {
        min: Option<NumberFile>,
        max: Option<NumberFile>,
    },
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct BonusParameters {
    pub extra: Option<i32>,
    pub probability: Option<f32>,
    #[serde(rename = "bonusMultiplier")]
    pub bonus_multiplier: Option<i32>,
}
