use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_tag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::Profile;
use crate::entity_data::ENTITY_TYPE_KEY;
use crate::saved_form::Fault;
use crate::saved_key::REMOVAL_MARKER;
use crate::saved_sink::Field;
use crate::saved_sink::KeyedEntry;
use crate::saved_sink::Leaf;
use crate::saved_sink::MapKey;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::TAG_MARKER;
use crate::saved_sink::WireOnly;

pub(crate) struct TagSink;

impl SavedSink for TagSink {
    type Node = NbtTag;

    const WIRE_ONLY: WireOnly = WireOnly::Refuse;

    fn leaf(&self, leaf: Leaf<'_>) -> NbtTag {
        match leaf {
            Leaf::Byte(value) => NbtTag::Byte(value),
            Leaf::Int(value) => NbtTag::Int(value),
            Leaf::Long(value) => NbtTag::Long(value),
            Leaf::IntArray(values) => NbtTag::IntArray(values.to_vec()),
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

    fn profile(&mut self, profile: &Profile) -> Option<NbtTag> {
        to_nbt_tag(profile).ok()
    }

    fn compound(&mut self, compound: &NbtCompound) -> Option<NbtTag> {
        Some(NbtTag::Compound(compound.clone()))
    }

    fn tag(&mut self, tag: &NbtTag) -> Option<NbtTag> {
        Some(tag.clone())
    }

    fn entity_data(
        &mut self,
        data: &NbtCompound,
        entity_type: ResourceLocation<'_>,
    ) -> Option<NbtTag> {
        let mut compound = data.clone();
        compound.put(ENTITY_TYPE_KEY, NbtTag::String(entity_type.to_string()));
        Some(NbtTag::Compound(compound))
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

    fn keyed_map<'a, T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut entry: impl FnMut(&mut Self, T) -> Result<KeyedEntry<'a, NbtTag>, Fault>,
    ) -> Result<NbtTag, Fault> {
        let items = items.into_iter();
        let mut child_tags: Vec<(String, NbtTag)> = Vec::with_capacity(items.size_hint().0);
        for item in items {
            entry(self, item).map(|entry| push_keyed(&mut child_tags, entry))?;
        }
        Ok(NbtTag::Compound(NbtCompound { child_tags }))
    }
}

#[inline(never)]
fn push_keyed(child_tags: &mut Vec<(String, NbtTag)>, entry: KeyedEntry<'_, NbtTag>) {
    let KeyedEntry { key, value } = entry;
    let key = match key {
        MapKey::String(key) => key.to_string(),
        MapKey::Name(name) => name.to_string(),
        MapKey::Removal(name) => format!("{REMOVAL_MARKER}{name}"),
    };
    child_tags.push((key, value));
}
