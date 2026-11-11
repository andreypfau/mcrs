use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

/// An entry of the `trim_pattern` registry. `I` is the item type of the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "I: TextSerde", deserialize = "I: TextSerde"))]
pub struct TrimPattern<I: TextTypes> {
    pub asset_id: ResourceLocationBuf,
    #[serde(default)]
    pub decal: bool,
    pub description: Text<I>,
}
