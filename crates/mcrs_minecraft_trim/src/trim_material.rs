use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

/// An entry of the `trim_material` registry. `I` is the item type of the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "I: TextSerde", deserialize = "I: TextSerde"))]
pub struct TrimMaterial<I: TextTypes> {
    pub palette_id: ResourceLocationBuf,
    pub description: Text<I>,
}
