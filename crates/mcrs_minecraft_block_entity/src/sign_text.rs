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

/// The two sides of a sign.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignTexts {
    pub front_text: SignText,
    pub back_text: SignText,
}

/// The sides a chunk packet or a block entity update carries for a sign. Those compounds hold no
/// position, and a side they leave out is a side they do not change.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct SignUpdate {
    pub front_text: Option<SignText>,
    pub back_text: Option<SignText>,
}

impl SignUpdate {
    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        mcrs_minecraft_nbt::from_tag(NbtTag::Compound(compound.clone()))
    }

    /// `base` with the sides this update carries replaced.
    #[must_use]
    pub fn applied_to(self, base: SignTexts) -> SignTexts {
        SignTexts {
            front_text: self.front_text.unwrap_or(base.front_text),
            back_text: self.back_text.unwrap_or(base.back_text),
        }
    }
}

impl SignTexts {
    /// The sides of a sign that has no stored state to keep: a side the compound leaves out is
    /// the empty one.
    pub fn from_compound(compound: &NbtCompound) -> Result<Self, mcrs_minecraft_nbt::Error> {
        SignUpdate::from_compound(compound).map(|update| update.applied_to(Self::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_front() -> NbtCompound {
        let mut front = NbtCompound::new();
        front.put_list("messages", vec![NbtTag::String("a".to_owned()); 4]);
        front.put_string("color", "red".to_owned());
        front.put_byte("has_glowing_text", 1);
        let mut compound = NbtCompound::new();
        compound.put("front_text", NbtTag::Compound(front));
        compound
    }

    #[test]
    fn a_compound_without_a_position_reads_its_sides() {
        let texts = SignTexts::from_compound(&red_front()).unwrap();
        assert_eq!(texts.front_text.color, "red");
        assert!(texts.front_text.has_glowing_text);
        assert_eq!(texts.back_text, SignText::default());
    }

    #[test]
    fn a_side_the_update_leaves_out_keeps_the_stored_one() {
        let stored = SignTexts {
            front_text: SignText::default(),
            back_text: SignText {
                color: "blue".to_owned(),
                ..SignText::default()
            },
        };
        let update = SignUpdate::from_compound(&red_front()).unwrap();
        let merged = update.applied_to(stored.clone());
        assert_eq!(merged.front_text.color, "red");
        assert_eq!(merged.back_text, stored.back_text);
        let empty = SignUpdate::from_compound(&NbtCompound::new()).unwrap();
        assert_eq!(empty.applied_to(stored.clone()), stored);
    }
}
