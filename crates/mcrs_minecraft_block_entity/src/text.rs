use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

/// A chat component: a bare string when it is nothing but literal text, the
/// full compound otherwise.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Rich(NbtTag),
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match <NbtTag as Deserialize>::deserialize(deserializer)? {
            NbtTag::String(text) => Text::Plain(text),
            NbtTag::Compound(compound) => match compound.child_tags.as_slice() {
                [(key, NbtTag::String(text))] if key == "text" => Text::Plain(text.clone()),
                _ => Text::Rich(NbtTag::Compound(compound)),
            },
            tag => Text::Rich(tag),
        })
    }
}
