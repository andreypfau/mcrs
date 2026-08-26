use std::fmt;

use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

type Span = (u32, u32);

#[inline]
fn slice(text: &str, span: Span) -> &str {
    &text[span.0 as usize..(span.0 + span.1) as usize]
}

/// One text buffer and one span per entry, rather than an owned `String` per
/// name and a map per property set. A region holds roughly three quarters of a
/// million palette entries, so the difference is allocator traffic, not bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette {
    text: String,
    entries: Vec<Entry>,
    props: Vec<(Span, Span)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    name: Span,
    props: Span,
}

impl Palette {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
/// name as borrowed text. Implemented by whoever owns the block registry; this
/// crate never sees one.
pub trait BlockStateLookup {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u32>;
}

impl<'de> Deserialize<'de> for Palette {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(PaletteVisitor)
    }
}

struct PaletteVisitor;

impl<'de> Visitor<'de> for PaletteVisitor {
    type Value = Palette;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a palette list")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Palette, A::Error> {
        let mut palette = Palette::default();
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
    palette: &'p mut Palette,
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
    palette: &'p mut Palette,
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
                Field::Name => {
                    let span = map.next_value_seed(TextSeed {
                        text: &mut self.palette.text,
                    })?;
                    name = Some(span);
                }
                Field::Properties => {
                    let Palette { text, props, .. } = &mut *self.palette;
                    map.next_value_seed(PropertiesSeed { text, props })?;
                }
            }
        }

        let name = name.ok_or_else(|| A::Error::missing_field("Name"))?;
        let props = (start, self.palette.props.len() as u32 - start);
        self.palette.entries.push(Entry { name, props });
        Ok(())
    }
}

enum Field {
    Name,
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
        f.write_str("`Name` or `Properties`")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Field, E> {
        match value {
            "Name" => Ok(Field::Name),
            "Properties" => Ok(Field::Properties),
            _ => Err(E::unknown_field(value, &["Name", "Properties"])),
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
