use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::condition_file::ConditionFile;
use crate::dispatch_case_file::DispatchCaseFile;
use crate::distribution_file::DistributionFile;
use crate::environment_read::{EnvironmentRead, int_environment};
use crate::float_provider_file::FloatProviderFile;
use crate::nbt_path::nbt_path;
use crate::provider_form::{NumberValue, ProviderForm};
use crate::provider_set_file::ProviderSetFile;
use crate::score_target_file::ScoreTargetFile;

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct IntProviderFile(pub(crate) ProviderForm<i32, TypedInt>);

impl mcrs_minecraft_resource::RegistryValue for IntProviderFile {
    type Registry = mcrs_minecraft_loot::loot::ContextIntProvider;
}

mcrs_minecraft_registries::resolve_as_is!(IntProviderFile);

type Int = IntProviderFile;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TypedInt {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant { value: NumberValue<i32> },
    #[serde(rename = "minecraft:abs", alias = "abs")]
    Abs { input: Int },
    #[serde(rename = "minecraft:negate", alias = "negate")]
    Negate { input: Int },
    #[serde(rename = "minecraft:from_float", alias = "from_float")]
    FromFloat { input: FloatProviderFile },
    #[serde(rename = "minecraft:sub", alias = "sub")]
    Sub { left: Int, right: Int },
    #[serde(rename = "minecraft:mod", alias = "mod")]
    Mod { left: Int, right: Int },
    #[serde(rename = "minecraft:floor_mod", alias = "floor_mod")]
    FloorMod { left: Int, right: Int },
    #[serde(rename = "minecraft:div", alias = "div")]
    Div { left: Int, right: Int },
    #[serde(rename = "minecraft:floor_div", alias = "floor_div")]
    FloorDiv { left: Int, right: Int },
    #[serde(rename = "minecraft:pow", alias = "pow")]
    Pow { base: Int, exponent: Int },
    #[serde(rename = "minecraft:uniform", alias = "uniform")]
    Uniform { min: Int, max: Int },
    #[serde(rename = "minecraft:add", alias = "add")]
    Add { inputs: ProviderSetFile<Int> },
    #[serde(rename = "minecraft:mul", alias = "mul")]
    Mul { inputs: ProviderSetFile<Int> },
    #[serde(rename = "minecraft:min", alias = "min")]
    Min { inputs: ProviderSetFile<Int> },
    #[serde(rename = "minecraft:max", alias = "max")]
    Max { inputs: ProviderSetFile<Int> },
    #[serde(rename = "minecraft:avg", alias = "avg")]
    Avg { inputs: ProviderSetFile<Int> },
    #[serde(rename = "minecraft:conditional", alias = "conditional")]
    Conditional {
        condition: ConditionFile,
        on_true: Int,
        on_false: Option<Int>,
    },
    #[serde(rename = "minecraft:number_dispatcher", alias = "number_dispatcher")]
    NumberDispatcher {
        cases: Vec<DispatchCaseFile<Int>>,
        default: Option<Int>,
    },
    #[serde(rename = "minecraft:weighted_list", alias = "weighted_list")]
    WeightedList { distribution: DistributionFile<Int> },
    #[serde(
        rename = "minecraft:environment_attribute",
        alias = "environment_attribute"
    )]
    EnvironmentAttribute {
        #[serde(deserialize_with = "int_environment")]
        attribute: EnvironmentRead<i32>,
    },
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage {
        storage: ResourceLocationBuf,
        #[serde(deserialize_with = "nbt_path")]
        path: NbtPath,
        fallback: Option<Int>,
    },
    #[serde(rename = "minecraft:binomial", alias = "binomial")]
    Binomial { n: Int, p: FloatProviderFile },
    #[serde(rename = "minecraft:score", alias = "score")]
    Score {
        target: ScoreTargetFile,
        score: String,
        fallback: Option<Int>,
    },
}
