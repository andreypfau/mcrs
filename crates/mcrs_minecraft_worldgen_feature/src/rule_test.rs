use mcrs_minecraft_registries::dispatch;
use mcrs_minecraft_resource::ResourceLocationBuf;

use mcrs_minecraft_block_predicate::block_state::BlockState;

#[dispatch(registry = crate::keys::RuleTestType, key = "predicate_type")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum RuleTest {
    AlwaysTrue,
    BlockMatch {
        block: ResourceLocationBuf,
    },
    #[dispatch(BlockstateMatch)]
    BlockStateMatch {
        block_state: BlockState,
    },
    TagMatch {
        tag: ResourceLocationBuf,
    },
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    RandomBlockMatch {
        block: ResourceLocationBuf,
        probability: f32,
    },
    #[dispatch(RandomBlockstateMatch)]
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
