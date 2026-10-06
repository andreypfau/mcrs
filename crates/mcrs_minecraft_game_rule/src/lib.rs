#[rustfmt::skip]
pub mod keys;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceLocation;
use serde::Deserialize;

pub use keys::GameRule;

const DEFINITIONS_JSON: &str =
    include_str!("../../../assets/mcrs/registry_definition/game_rule.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum GameRuleCategory {
    #[serde(rename = "minecraft:player")]
    Player,
    #[serde(rename = "minecraft:mobs")]
    Mobs,
    #[serde(rename = "minecraft:spawning")]
    Spawning,
    #[serde(rename = "minecraft:drops")]
    Drops,
    #[serde(rename = "minecraft:updates")]
    Updates,
    #[serde(rename = "minecraft:chat")]
    Chat,
    #[serde(rename = "minecraft:misc")]
    Misc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRuleValueType {
    Bool { default: bool },
    Int { default: i32, min: i32, max: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "DefinitionFile")]
pub struct GameRuleDefinition {
    pub category: GameRuleCategory,
    pub value: GameRuleValueType,
    pub required_features: Box<[ResourceLocation]>,
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum DefinitionFile {
    #[serde(rename = "boolean")]
    Bool {
        category: GameRuleCategory,
        default: bool,
        #[serde(default)]
        required_features: Box<[ResourceLocation]>,
    },
    #[serde(rename = "integer")]
    Int {
        category: GameRuleCategory,
        default: i32,
        min: i32,
        max: i32,
        #[serde(default)]
        required_features: Box<[ResourceLocation]>,
    },
}

impl From<DefinitionFile> for GameRuleDefinition {
    fn from(file: DefinitionFile) -> Self {
        match file {
            DefinitionFile::Bool {
                category,
                default,
                required_features,
            } => GameRuleDefinition {
                category,
                value: GameRuleValueType::Bool { default },
                required_features,
            },
            DefinitionFile::Int {
                category,
                default,
                min,
                max,
                required_features,
            } => GameRuleDefinition {
                category,
                value: GameRuleValueType::Int { default, min, max },
                required_features,
            },
        }
    }
}

static DEFINITIONS: LazyLock<Box<[GameRuleDefinition]>> = LazyLock::new(|| {
    let definitions: BTreeMap<GameRule, GameRuleDefinition> =
        serde_json::from_str(DEFINITIONS_JSON)
            .unwrap_or_else(|e| panic!("the dumped game rule definitions do not read: {e}"));
    let missing: Vec<_> = GameRule::ALL
        .iter()
        .filter(|rule| !definitions.contains_key(rule))
        .map(|rule| rule.as_static_str())
        .collect();
    assert!(
        missing.is_empty(),
        "the dumped game rule definitions lack {missing:?}"
    );
    definitions.into_values().collect()
});

impl GameRule {
    pub fn definition(self) -> &'static GameRuleDefinition {
        &DEFINITIONS[self as usize]
    }
}
