use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Deserialize;

use crate::keys;

pub const DEFINITIONS: &str = "assets/mcrs/registry_definition/game_rule.json";
pub const ENUM_FILE: &str = "crates/mcrs_minecraft_game_rule/src/keys/game_rule.rs";
const REGISTRY: &str = "minecraft:game_rule";

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum Definition {
    #[serde(rename = "boolean")]
    Bool {
        category: String,
        default: bool,
        #[serde(default)]
        required_features: Vec<String>,
    },
    #[serde(rename = "integer")]
    Int {
        category: String,
        default: i32,
        min: i32,
        max: i32,
        #[serde(default)]
        required_features: Vec<String>,
    },
}

/// `GameRule::definition` as one exhaustive match, so a rule the dump lacks
/// fails the build of the game rule crate.
pub fn definition_impl(json: &str) -> Result<String, String> {
    let definitions: BTreeMap<String, Definition> =
        serde_json::from_str(json).map_err(|error| format!("{DEFINITIONS}: {error}"))?;
    let mut out = String::from(
        "\nuse crate::{GameRuleCategory, GameRuleDefinition, GameRuleValueType};\n\
         use mcrs_minecraft_resource::rl;\n\n\
         impl GameRule {\n\
         #[expect(clippy::too_many_lines, reason = \"one arm per game rule the game ships\")]\n\
         pub const fn definition(self) -> &'static GameRuleDefinition {\n\
         match self {\n",
    );
    for (name, definition) in &definitions {
        let rule = keys::variant(&keys::constant(REGISTRY, name)?);
        let (category, value, features) = match definition {
            Definition::Bool {
                category,
                default,
                required_features,
            } => (
                category,
                format!("GameRuleValueType::Bool {{ default: {default} }}"),
                required_features,
            ),
            Definition::Int {
                category,
                default,
                min,
                max,
                required_features,
            } => (
                category,
                format!("GameRuleValueType::Int {{ default: {default}, min: {min}, max: {max} }}"),
                required_features,
            ),
        };
        let category = category
            .strip_prefix("minecraft:")
            .map(|path| keys::variant(&path.to_ascii_uppercase()))
            .ok_or_else(|| {
                format!("{DEFINITIONS}: {name}: category {category} is not minecraft:")
            })?;
        let features: String = features
            .iter()
            .map(|feature| format!("rl!(\"{feature}\"), "))
            .collect();
        let _ = write!(
            out,
            "Self::{rule} => &GameRuleDefinition {{\n\
             category: GameRuleCategory::{category},\n\
             value: {value},\n\
             required_features: &[{features}],\n\
             }},\n"
        );
    }
    out.push_str("}\n}\n}\n");
    Ok(out)
}
