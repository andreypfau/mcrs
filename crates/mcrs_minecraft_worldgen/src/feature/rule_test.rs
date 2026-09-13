use serde::{Deserialize, Serialize};

use crate::proto::BlockState;
use mcrs_minecraft_core::ResourceLocation;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "predicate_type", deny_unknown_fields)]
pub enum RuleTest {
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:block_match")]
    BlockMatch { block: ResourceLocation },
    #[serde(rename = "minecraft:blockstate_match")]
    BlockStateMatch { block_state: BlockState },
    #[serde(rename = "minecraft:tag_match")]
    TagMatch { tag: ResourceLocation },
    #[serde(rename = "minecraft:height_match")]
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:random_block_match")]
    RandomBlockMatch {
        block: ResourceLocation,
        probability: f32,
    },
    #[serde(rename = "minecraft:random_blockstate_match")]
    RandomBlockStateMatch {
        block_state: BlockState,
        probability: f32,
    },
    #[serde(rename = "minecraft:all_of")]
    AllOf { rules: Vec<RuleTest> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { rules: Vec<RuleTest> },
    #[serde(rename = "minecraft:not")]
    Not { rule: Box<RuleTest> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(json: &str) {
        let parsed: RuleTest = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn every_shipped_shape_round_trips() {
        round_trip(r#"{"predicate_type":"minecraft:always_true"}"#);
        round_trip(r#"{"predicate_type":"minecraft:block_match","block":"minecraft:netherrack"}"#);
        round_trip(
            r#"{"predicate_type":"minecraft:blockstate_match","block_state":{"id":"minecraft:snow","properties":{"layers":"1"}}}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:tag_match","tag":"minecraft:stone_ore_replaceables"}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:height_match","min_inclusive":-2032,"max_inclusive":8}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:random_block_match","block":"minecraft:blackstone","probability":0.01}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:random_blockstate_match","block_state":"minecraft:gravel","probability":0.33333334}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:not","rule":{"predicate_type":"minecraft:always_true"}}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:all_of","rules":[{"predicate_type":"minecraft:always_true"}]}"#,
        );
        round_trip(
            r#"{"predicate_type":"minecraft:any_of","rules":[{"predicate_type":"minecraft:always_true"}]}"#,
        );
    }

    #[test]
    fn an_unregistered_type_is_a_load_error() {
        let error = serde_json::from_str::<RuleTest>(
            r#"{"predicate_type":"minecraft:axis_aligned_linear_pos"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("minecraft:axis_aligned_linear_pos"),
            "{error}"
        );
    }
}
