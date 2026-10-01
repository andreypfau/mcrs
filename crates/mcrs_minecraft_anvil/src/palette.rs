use std::fmt;

use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

type Span = (u32, u32);

#[inline]
fn slice(text: &str, span: Span) -> &str {
    &text[span.0 as usize..(span.0 + span.1) as usize]
}

/// One text buffer and one span per entry, rather than an owned `String` per
/// name and a map per property set. A region holds roughly three quarters of a
/// million palette entries, so the difference is allocator traffic, not bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockStateList {
    text: String,
    entries: Vec<Entry>,
    props: Vec<(Span, Span)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    name: Span,
    props: Span,
}

impl BlockStateList {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn name(&self, index: usize) -> &str {
        slice(&self.text, self.entries[index].name)
    }

    pub fn properties(&self, index: usize) -> Properties<'_> {
        let entry = self.entries[index];
        let start = entry.props.0 as usize;
        Properties {
            text: &self.text,
            props: &self.props[start..start + entry.props.1 as usize],
        }
    }

    fn push_str(&mut self, value: &str) -> Span {
        let start = self.text.len() as u32;
        self.text.push_str(value);
        (start, value.len() as u32)
    }

    pub(crate) fn push<'p>(
        &mut self,
        name: &str,
        properties: impl IntoIterator<Item = (&'p str, &'p str)>,
    ) {
        let name = self.push_str(name);
        let start = self.props.len() as u32;
        for (key, value) in properties {
            let key = self.push_str(key);
            let value = self.push_str(value);
            self.props.push((key, value));
        }
        let props = (start, self.props.len() as u32 - start);
        self.entries.push(Entry { name, props });
    }
}

/// Vanilla resolves a state by asking the block for each property it declares,
/// so lookup is by key and the stored order never matters. Entries are few, so
/// a scan beats anything with a hash.
#[derive(Debug, Clone, Copy)]
pub struct Properties<'a> {
    text: &'a str,
    props: &'a [(Span, Span)],
}

impl<'a> Properties<'a> {
    pub fn len(&self) -> usize {
        self.props.len()
    }

    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&'a str> {
        self.props
            .iter()
            .find(|(k, _)| slice(self.text, *k) == key)
            .map(|(_, v)| slice(self.text, *v))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.props
            .iter()
            .map(|&(k, v)| (slice(self.text, k), slice(self.text, v)))
    }
}

/// Turns a palette entry into a registry id while the decoder still holds the
/// name as borrowed text. Implemented by whoever owns the registry; this crate
/// never sees one.
pub trait PaletteLookup<V> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<V>;
}

impl<'de> Deserialize<'de> for BlockStateList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(PaletteVisitor)
    }
}

/// An entry without properties is a bare name, which is how vanilla writes a
/// block's default state and every biome; any other is `{ id, properties }`.
impl Serialize for BlockStateList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for index in 0..self.len() {
            let properties = self.properties(index);
            if properties.is_empty() {
                seq.serialize_element(self.name(index))?;
            } else {
                seq.serialize_element(&BlockState {
                    id: self.name(index),
                    properties,
                })?;
            }
        }
        seq.end()
    }
}

struct BlockState<'a> {
    id: &'a str,
    properties: Properties<'a>,
}

impl Serialize for BlockState<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("BlockState", 2)?;
        state.serialize_field("id", self.id)?;
        state.serialize_field("properties", &self.properties)?;
        state.end()
    }
}

impl Serialize for Properties<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

struct PaletteVisitor;

impl<'de> Visitor<'de> for PaletteVisitor {
    type Value = BlockStateList;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a palette list")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<BlockStateList, A::Error> {
        let mut palette = BlockStateList::default();
        if let Some(hint) = seq.size_hint() {
            palette.entries.reserve(hint);
        }
        while seq
            .next_element_seed(EntrySeed {
                palette: &mut palette,
            })?
            .is_some()
        {}
        Ok(palette)
    }
}

struct EntrySeed<'p> {
    palette: &'p mut BlockStateList,
}

impl<'de> DeserializeSeed<'de> for EntrySeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(EntryVisitor {
            palette: self.palette,
        })
    }
}

struct EntryVisitor<'p> {
    palette: &'p mut BlockStateList,
}

impl<'de> Visitor<'de> for EntryVisitor<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a block state or a biome name")
    }

    /// A biome palette is a list of bare names.
    fn visit_str<E: Error>(self, value: &str) -> Result<(), E> {
        let name = self.palette.push_str(value);
        let props = (self.palette.props.len() as u32, 0);
        self.palette.entries.push(Entry { name, props });
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let start = self.palette.props.len() as u32;
        let mut name: Option<Span> = None;

        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::Id => {
                    let span = map.next_value_seed(TextSeed {
                        text: &mut self.palette.text,
                    })?;
                    name = Some(span);
                }
                Field::Properties => {
                    let BlockStateList { text, props, .. } = &mut *self.palette;
                    map.next_value_seed(PropertiesSeed { text, props })?;
                }
            }
        }

        let name = name.ok_or_else(|| A::Error::missing_field("id"))?;
        let props = (start, self.palette.props.len() as u32 - start);
        self.palette.entries.push(Entry { name, props });
        Ok(())
    }
}

enum Field {
    Id,
    Properties,
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_identifier(FieldVisitor)
    }
}

struct FieldVisitor;

impl Visitor<'_> for FieldVisitor {
    type Value = Field;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("`id` or `properties`")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Field, E> {
        match value {
            "id" => Ok(Field::Id),
            "properties" => Ok(Field::Properties),
            _ => Err(E::unknown_field(value, &["id", "properties"])),
        }
    }
}

/// Appends one string to the arena and answers where it landed, so the value
/// never becomes an owned `String` on the way.
struct TextSeed<'p> {
    text: &'p mut String,
}

impl<'de> DeserializeSeed<'de> for TextSeed<'_> {
    type Value = Span;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Span, D::Error> {
        deserializer.deserialize_str(TextVisitor { text: self.text })
    }
}

struct TextVisitor<'p> {
    text: &'p mut String,
}

impl Visitor<'_> for TextVisitor<'_> {
    type Value = Span;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Span, E> {
        let start = self.text.len() as u32;
        self.text.push_str(value);
        Ok((start, value.len() as u32))
    }
}

struct PropertiesSeed<'p> {
    text: &'p mut String,
    props: &'p mut Vec<(Span, Span)>,
}

impl<'de> DeserializeSeed<'de> for PropertiesSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for PropertiesSeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a property map")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key_seed(TextSeed { text: self.text })? {
            let value = map.next_value_seed(TextSeed { text: self.text })?;
            self.props.push((key, value));
        }
        Ok(())
    }
}
