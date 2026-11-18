use serde::{Deserialize, Serialize};

use crate::deserializer::NbtReadHelper;
use crate::encoding::NbtEncoding;
use crate::serializer::WriteAdaptor;
use crate::tag::NbtTag;
use crate::tag::deepest;
use crate::{END_ID, Error, Nbt, get_nbt_string};
use std::io::{Read, Seek, Write};
use std::vec::IntoIter;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NbtCompound {
    pub child_tags: Vec<(String, NbtTag)>,
}

impl NbtCompound {
    pub const fn new() -> NbtCompound {
        NbtCompound {
            child_tags: Vec::new(),
        }
    }

    pub fn skip_content<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
    ) -> Result<(), Error> {
        loop {
            let tag_id = reader.read_entry_kind()?;
            if tag_id == END_ID {
                break;
            }

            let len = reader.read_string_length()?;
            reader.skip_bytes(len as i64)?;

            NbtTag::skip_data(reader, tag_id)?;
        }

        Ok(())
    }

    pub fn deserialize_content<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
    ) -> Result<NbtCompound, Error> {
        let mut child_tags = Vec::new();

        loop {
            let tag_id = reader.read_entry_kind()?;
            if tag_id == END_ID {
                break;
            }

            let name = get_nbt_string(reader)?;
            NbtTag::deserialize_data(reader, tag_id).map(|tag| child_tags.push((name, tag)))?;
        }

        Ok(NbtCompound {
            child_tags: fold_duplicate_keys(child_tags),
        })
    }

    pub fn serialize_content<W: Write, E: NbtEncoding>(
        &self,
        w: &mut WriteAdaptor<W, E>,
    ) -> Result<(), Error> {
        for (name, tag) in &self.child_tags {
            w.write_u8(tag.get_type_id())?;
            E::write_string(w, name)?;
            tag.serialize_data(w)?;
        }
        w.write_u8(END_ID)?;
        Ok(())
    }

    #[must_use]
    pub fn depth(&self, budget: u32) -> u32 {
        let Some(budget) = budget.checked_sub(1) else {
            return 1;
        };
        deepest(self.child_tags.iter().map(|(_, child)| child.depth(budget)))
    }

    pub fn matches(&self, actual: &NbtCompound) -> bool {
        self.child_tags.iter().all(|(key, expected)| {
            actual
                .get(key)
                .is_some_and(|actual| expected.matches(actual))
        })
    }

    pub fn merge(&mut self, source: &NbtCompound) {
        let mut pending: Vec<(&mut NbtCompound, &NbtCompound)> = vec![(self, source)];
        while let Some((target, source)) = pending.pop() {
            for (key, value) in &source.child_tags {
                let existing = target
                    .child_tags
                    .iter_mut()
                    .find(|(existing_key, _)| existing_key == key);
                let copy = match value {
                    NbtTag::Compound(_) => NbtTag::Compound(NbtCompound::new()),
                    _ => value.clone(),
                };
                match existing {
                    Some((_, NbtTag::Compound(_))) if matches!(value, NbtTag::Compound(_)) => {}
                    Some((_, existing)) => *existing = copy,
                    None => target.child_tags.push((key.clone(), copy)),
                }
            }
            for (key, value) in target.child_tags.iter_mut() {
                if let NbtTag::Compound(target_value) = value
                    && let Some(NbtTag::Compound(source_value)) = source.get(key)
                {
                    pending.push((target_value, source_value));
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.child_tags.is_empty()
    }

    pub fn put(&mut self, name: &str, value: impl Into<NbtTag>) {
        let value = value.into();
        match self.child_tags.iter_mut().find(|(key, _)| key == name) {
            Some(entry) => entry.1 = value,
            None => self.child_tags.push((name.to_string(), value)),
        }
    }

    pub fn put_string(&mut self, name: &str, value: String) {
        self.put(name, NbtTag::String(value));
    }

    pub fn put_list(&mut self, name: &str, value: Vec<NbtTag>) {
        self.put(name, NbtTag::List(value));
    }

    pub fn put_byte(&mut self, name: &str, value: i8) {
        self.put(name, NbtTag::Byte(value));
    }

    pub fn put_bool(&mut self, name: &str, value: bool) {
        self.put(name, NbtTag::Byte(if value { 1 } else { 0 }));
    }

    pub fn put_short(&mut self, name: &str, value: i16) {
        self.put(name, NbtTag::Short(value));
    }

    pub fn put_int(&mut self, name: &str, value: i32) {
        self.put(name, NbtTag::Int(value));
    }
    pub fn put_long(&mut self, name: &str, value: i64) {
        self.put(name, NbtTag::Long(value));
    }

    pub fn put_float(&mut self, name: &str, value: f32) {
        self.put(name, NbtTag::Float(value));
    }

    pub fn put_double(&mut self, name: &str, value: f64) {
        self.put(name, NbtTag::Double(value));
    }

    pub fn put_component(&mut self, name: &str, value: NbtCompound) {
        self.put(name, NbtTag::Compound(value));
    }

    pub fn get_byte(&self, name: &str) -> Option<i8> {
        self.get(name).and_then(|tag| tag.extract_byte())
    }

    #[inline]
    pub fn get(&self, name: &str) -> Option<&NbtTag> {
        self.child_tags
            .iter()
            .find(|k| k.0.as_str() == name)
            .map(|r| &r.1)
    }

    pub fn get_short(&self, name: &str) -> Option<i16> {
        self.get(name).and_then(|tag| tag.extract_short())
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.get(name).and_then(|tag| tag.extract_int())
    }

    pub fn get_long(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(|tag| tag.extract_long())
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(|tag| tag.extract_bool())
    }

    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(|tag| tag.extract_string())
    }

    pub fn get_list(&self, name: &str) -> Option<&[NbtTag]> {
        self.get(name).and_then(|tag| tag.extract_list())
    }

    pub fn get_compound(&self, name: &str) -> Option<&NbtCompound> {
        self.get(name).and_then(|tag| tag.extract_compound())
    }

    pub fn get_int_array(&self, name: &str) -> Option<&[i32]> {
        self.get(name).and_then(|tag| tag.extract_int_array())
    }
}

impl From<Nbt> for NbtCompound {
    fn from(value: Nbt) -> Self {
        value.root_tag
    }
}

impl FromIterator<(String, NbtTag)> for NbtCompound {
    fn from_iter<T: IntoIterator<Item = (String, NbtTag)>>(iter: T) -> Self {
        let mut compound = NbtCompound::new();
        for (key, value) in iter {
            compound.put(&key, value);
        }
        compound
    }
}

impl IntoIterator for NbtCompound {
    type Item = (String, NbtTag);
    type IntoIter = IntoIter<(String, NbtTag)>;

    fn into_iter(self) -> Self::IntoIter {
        self.child_tags.into_iter()
    }
}

impl Extend<(String, NbtTag)> for NbtCompound {
    fn extend<T: IntoIterator<Item = (String, NbtTag)>>(&mut self, iter: T) {
        self.child_tags.extend(iter)
    }
}

impl AsRef<NbtCompound> for NbtCompound {
    fn as_ref(&self) -> &NbtCompound {
        self
    }
}

impl Serialize for NbtCompound {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.child_tags.len()))?;
        for (key, value) in &self.child_tags {
            map.serialize_entry(key, &value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for NbtCompound {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CompoundVisitor;

        impl<'de> serde::de::Visitor<'de> for CompoundVisitor {
            type Value = NbtCompound;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an NBT compound")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut compound = NbtCompound::new();
                while let Some(key) = map.next_key::<String>()? {
                    map.next_value::<NbtTag>()
                        .map(|value| compound.put(&key, value))?;
                }
                Ok(compound)
            }
        }

        deserializer.deserialize_map(CompoundVisitor)
    }
}

impl From<NbtCompound> for NbtTag {
    fn from(value: NbtCompound) -> Self {
        NbtTag::Compound(value)
    }
}

fn fold_duplicate_keys(child_tags: Vec<(String, NbtTag)>) -> Vec<(String, NbtTag)> {
    if child_tags.len() < 2 {
        return child_tags;
    }
    let mut by_key: Vec<(&str, usize)> = child_tags
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (key.as_str(), index))
        .collect();
    by_key.sort_unstable();
    if by_key
        .windows(2)
        .all(|pair| matches!(pair, [first, second] if first.0 != second.0))
    {
        return child_tags;
    }
    let mut value_source: Vec<Option<usize>> = vec![None; child_tags.len()];
    for group in by_key.chunk_by(|first, second| first.0 == second.0) {
        if let (Some(first), Some(last)) = (group.first(), group.last())
            && let Some(source) = value_source.get_mut(first.1)
        {
            *source = Some(last.1);
        }
    }
    let mut slots: Vec<Option<(String, NbtTag)>> = child_tags.into_iter().map(Some).collect();
    let mut folded = Vec::with_capacity(slots.len());
    for (position, source) in value_source.into_iter().enumerate() {
        let Some(source) = source else {
            continue;
        };
        let Some((key, first_value)) = slots.get_mut(position).and_then(Option::take) else {
            continue;
        };
        let value = slots
            .get_mut(source)
            .and_then(Option::take)
            .map_or(first_value, |(_, last_value)| last_value);
        folded.push((key, value));
    }
    folded
}
