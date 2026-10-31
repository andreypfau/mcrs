use mcrs_minecraft_registries::dispatch;

use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_resource::ResourceLocation;

#[dispatch(registry = crate::keys::RuleTestType, key = "predicate_type")]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum RuleTest {
    AlwaysTrue,
    BlockMatch {
        block: ResourceLocation,
    },
    #[dispatch(BlockstateMatch)]
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
