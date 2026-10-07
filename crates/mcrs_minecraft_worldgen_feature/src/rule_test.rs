use serde::{Deserialize, Serialize};

use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_core::ResourceLocation;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum RuleTest {
    AlwaysTrue,
    BlockMatch {
        block: ResourceLocation,
    },
    BlockStateMatch {
        block_state: BlockState,
    },
    TagMatch {
        tag: ResourceLocation,
    },
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    RandomBlockMatch {
        block: ResourceLocation,
        probability: f32,
    },
    RandomBlockStateMatch {
        block_state: BlockState,
        probability: f32,
    },
    AllOf {
        rules: Vec<RuleTest>,
    },
    AnyOf {
        rules: Vec<RuleTest>,
    },
    Not {
        rule: Box<RuleTest>,
    },
}

mcrs_minecraft_registry::dispatch! {
    RuleTest, key = "predicate_type", registry = crate::keys::RuleTestType,
    {
        AllOf => AllOf,
        AlwaysTrue => AlwaysTrue,
        AnyOf => AnyOf,
        BlockMatch => BlockMatch,
        BlockstateMatch => BlockStateMatch,
        HeightMatch => HeightMatch,
        Not => Not,
        RandomBlockMatch => RandomBlockMatch,
        RandomBlockstateMatch => RandomBlockStateMatch,
        TagMatch => TagMatch,
    }
}
