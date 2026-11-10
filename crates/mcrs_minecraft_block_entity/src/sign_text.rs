use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_flag;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

use crate::Text;

fn black() -> String {
    "black".to_owned()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignText {
    pub messages: Vec<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtered_messages: Option<Vec<Text>>,
    #[serde(default = "black")]
    pub color: String,
    #[serde(default, deserialize_with = "nbt_flag")]
    pub has_glowing_text: bool,
}

impl Default for SignText {
    fn default() -> Self {
        Self {
            messages: vec![Text::Plain(String::new()); 4],
            filtered_messages: None,
            color: black(),
            has_glowing_text: false,
        }
    }
}

/// The two sides of a sign as a chunk packet or a block entity update carries them. Those
/// compounds hold no position, so this reads only the side texts and leaves the rest alone.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SignTexts {
    #[serde(default)]
    pub front_text: SignText,
    #[serde(default)]
    pub back_text: SignText,
}

impl SignTexts {
    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        mcrs_minecraft_nbt::from_tag(NbtTag::Compound(compound.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compound_without_a_position_reads_its_sides() {
        let mut front = NbtCompound::new();
        front.put_list("messages", vec![NbtTag::String("a".to_owned()); 4]);
        front.put_string("color", "red".to_owned());
        front.put_byte("has_glowing_text", 1);
        let mut compound = NbtCompound::new();
        compound.put("front_text", NbtTag::Compound(front));
        let texts = SignTexts::from_compound(&compound).unwrap();
        assert_eq!(texts.front_text.color, "red");
        assert!(texts.front_text.has_glowing_text);
        assert_eq!(texts.back_text, SignText::default());
    }
}
