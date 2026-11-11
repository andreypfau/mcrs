use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_tag;
use mcrs_minecraft_text::Text;

use crate::saved_form::Fault;
use crate::saved_sink::Field;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::TAG_MARKER;

/// Builds the saved form as an NBT tree.
pub(crate) struct TagSink;

impl SavedSink for TagSink {
    type Node = NbtTag;

    fn leaf(&self, leaf: Leaf<'_>) -> NbtTag {
        match leaf {
            Leaf::Int(value) => NbtTag::Int(value),
            Leaf::Float(value) => NbtTag::Float(value),
            Leaf::Double(value) => NbtTag::Double(value),
            Leaf::Boolean(value) => NbtTag::Byte(i8::from(value)),
            Leaf::String(value) => NbtTag::String(value.into()),
            Leaf::Name(name) => NbtTag::String(name.to_string()),
            Leaf::TagName(name) => NbtTag::String(format!("{TAG_MARKER}{name}")),
        }
    }

    fn text(&mut self, text: &Text) -> Option<NbtTag> {
        to_nbt_tag(text).ok()
    }

    fn fields(&self, fields: impl IntoIterator<Item = Field<NbtTag>>) -> NbtTag {
        let mut map = NbtCompound::new();
        for field in fields {
            if let Some(value) = field.value {
                map.put(field.key, value);
            }
        }
        NbtTag::Compound(map)
    }

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<NbtTag, Fault> {
        let mut map = NbtCompound::new();
        for entry in entries {
            let entry = entry?;
            map.put(&entry.name.to_string(), self.leaf(entry.value));
        }
        Ok(NbtTag::Compound(map))
    }

    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut element: impl FnMut(&mut Self, T) -> Result<NbtTag, Fault>,
    ) -> Result<NbtTag, Fault> {
        let items = items.into_iter();
        let mut list: Vec<NbtTag> = Vec::with_capacity(items.size_hint().0);
        for item in items {
            list.push(element(self, item)?);
        }
        Ok(NbtTag::List(list))
    }
}
