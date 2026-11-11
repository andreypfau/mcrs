use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

/// An entry of the `painting_variant` registry. `I` is the item type of the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "I: TextSerde", deserialize = "I: TextSerde"))]
pub struct PaintingVariant<I: TextTypes> {
    pub width: Bounded<1, 16>,
    pub height: Bounded<1, 16>,
    pub asset_id: ResourceLocationBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Text<I>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Text<I>>,
}
