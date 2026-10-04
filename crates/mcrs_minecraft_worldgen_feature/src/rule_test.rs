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
