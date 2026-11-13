use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

/// A context number provider as a loot table writes it: a bare number, an inline object or the
/// key of a registered provider.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum NumberFile {
    Constant(f32),
    Typed(TypedNumber),
    Key(ResourceLocationBuf),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum TypedNumber {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant { value: f32 },
    #[serde(rename = "minecraft:uniform", alias = "uniform")]
    Uniform {
        min: Box<NumberFile>,
        max: Box<NumberFile>,
    },
    #[serde(rename = "minecraft:binomial", alias = "binomial")]
    Binomial {
        n: Box<NumberFile>,
        p: Box<NumberFile>,
    },
    #[serde(other)]
    Other,
}
