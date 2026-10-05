use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::proto::BlockState;

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

const RULE_TEST_TYPE_ROWS: &[&str] = &[
    "minecraft:always_true",
    "minecraft:block_match",
    "minecraft:blockstate_match",
    "minecraft:tag_match",
    "minecraft:height_match",
    "minecraft:random_block_match",
    "minecraft:random_blockstate_match",
    "minecraft:all_of",
    "minecraft:any_of",
    "minecraft:not",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    RULE_TEST_TYPE_ROWS,
    &[],
    mcrs_minecraft_keys::rule_test_type::NAMES
));

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn rule_test_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<RuleTest>(
            RULE_TEST_TYPE_ROWS,
            &[],
            mcrs_minecraft_keys::rule_test_type::NAMES,
            |name| serde_json::json!({ "predicate_type": name }),
        );
    }
}
