use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::condition_file::ConditionFile;
use crate::dispatch_case_file::DispatchCaseFile;
use crate::distribution_file::DistributionFile;
use crate::environment_read::{EnvironmentRead, float_environment};
use crate::float_provider::EnvironmentNumber;
use crate::int_provider_file::IntProviderFile;
use crate::level_based::checked_level_based;
use crate::nbt_path::nbt_path;
use crate::provider_form::{NumberValue, ProviderForm};
use crate::provider_set_file::ProviderSetFile;

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct FloatProviderFile(pub(crate) ProviderForm<f32, TypedFloat>);

impl mcrs_minecraft_resource::RegistryValue for FloatProviderFile {
    type Registry = mcrs_minecraft_loot::loot::ContextFloatProvider;
}

mcrs_minecraft_registries::resolve_as_is!(FloatProviderFile);

type Float = FloatProviderFile;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TypedFloat {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant { value: NumberValue<f32> },
    #[serde(rename = "minecraft:abs", alias = "abs")]
    Abs { input: Float },
    #[serde(rename = "minecraft:negate", alias = "negate")]
    Negate { input: Float },
    #[serde(rename = "minecraft:floor", alias = "floor")]
    Floor { input: Float },
    #[serde(rename = "minecraft:ceil", alias = "ceil")]
    Ceil { input: Float },
    #[serde(rename = "minecraft:round", alias = "round")]
    Round { input: Float },
    #[serde(rename = "minecraft:truncate", alias = "truncate")]
    Truncate { input: Float },
    #[serde(rename = "minecraft:sqrt", alias = "sqrt")]
    Sqrt { input: Float },
    #[serde(rename = "minecraft:sin", alias = "sin")]
    Sin { input: Float },
    #[serde(rename = "minecraft:cos", alias = "cos")]
    Cos { input: Float },
    #[serde(rename = "minecraft:from_int", alias = "from_int")]
    FromInt { input: IntProviderFile },
    #[serde(rename = "minecraft:sub", alias = "sub")]
    Sub { left: Float, right: Float },
    #[serde(rename = "minecraft:mod", alias = "mod")]
    Mod { left: Float, right: Float },
    #[serde(rename = "minecraft:div", alias = "div")]
    Div { left: Float, right: Float },
    #[serde(rename = "minecraft:pow", alias = "pow")]
    Pow { base: Float, exponent: Float },
    #[serde(rename = "minecraft:uniform", alias = "uniform")]
    Uniform { min: Float, max: Float },
    #[serde(rename = "minecraft:add", alias = "add")]
    Add { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:mul", alias = "mul")]
    Mul { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:min", alias = "min")]
    Min { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:max", alias = "max")]
    Max { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:avg", alias = "avg")]
    Avg { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:length", alias = "length")]
    Length { inputs: ProviderSetFile<Float> },
    #[serde(rename = "minecraft:conditional", alias = "conditional")]
    Conditional {
        condition: ConditionFile,
        on_true: Float,
        on_false: Option<Float>,
    },
    #[serde(rename = "minecraft:number_dispatcher", alias = "number_dispatcher")]
    NumberDispatcher {
        cases: Vec<DispatchCaseFile<Float>>,
        default: Option<Float>,
    },
    #[serde(rename = "minecraft:weighted_list", alias = "weighted_list")]
    WeightedList {
        distribution: DistributionFile<Float>,
    },
    #[serde(
        rename = "minecraft:environment_attribute",
        alias = "environment_attribute"
    )]
    EnvironmentAttribute {
        #[serde(deserialize_with = "float_environment")]
        attribute: EnvironmentRead<EnvironmentNumber>,
    },
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage {
        storage: ResourceLocationBuf,
        #[serde(deserialize_with = "nbt_path")]
        path: NbtPath,
        fallback: Option<Float>,
    },
    #[serde(rename = "minecraft:enchantment_level", alias = "enchantment_level")]
    EnchantmentLevel {
        #[serde(deserialize_with = "checked_level_based")]
        amount: LevelBasedValue,
    },
}
