use mcrs_minecraft_resource::ResourceLocationBuf;
use std::fmt;
use std::marker::PhantomData;
use std::vec;

use mcrs_minecraft_nbt::{ArrayKind, NBT_ARRAY_TAG};
use serde::de::{
    self, DeserializeSeed, EnumAccess, Error as _, IntoDeserializer, MapAccess, SeqAccess,
    Unexpected, VariantAccess, Visitor,
};
use serde::ser::{self, Impossible, SerializeMap, SerializeStruct, SerializeStructVariant};
use serde::{Deserialize, Deserializer, Serialize, Serializer, forward_to_deserialize_any};

#[doc(hidden)]
pub use mcrs_minecraft_resource as __resource;

/// A value read once and kept at the width and shape its format delivered, so
/// a codec can look at a map's keys before choosing how to read its values.
#[derive(Debug, Clone)]
pub enum Buffered {
    Unit,
    Absent,
    Present(Box<Buffered>),
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Char(char),
    Str(String),
    Bytes(Vec<u8>),
    Array(ArrayKind, Vec<u8>),
    Seq(Vec<Buffered>),
    Map(Vec<(Buffered, Buffered)>),
}

impl<'de> Deserialize<'de> for Buffered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_newtype_struct(NBT_ARRAY_TAG, Capture)
    }
}

struct Capture;

struct ArrayPayload;

impl<'de> DeserializeSeed<'de> for ArrayPayload {
    type Value = Vec<u8>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<u8>, D::Error> {
        deserializer.deserialize_byte_buf(self)
    }
}

impl<'de> Visitor<'de> for ArrayPayload {
    type Value = Vec<u8>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("the payload of an NBT array")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Vec<u8>, E> {
        Ok(value.to_vec())
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Vec<u8>, E> {
        Ok(value)
    }
}

macro_rules! capture_scalars {
    ($($method:ident($ty:ty) => $variant:ident),* $(,)?) => {$(
        fn $method<E>(self, value: $ty) -> Result<Buffered, E> {
            Ok(Buffered::$variant(value))
        }
    )*};
}

impl<'de> Visitor<'de> for Capture {
    type Value = Buffered;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any value")
    }

    capture_scalars! {
        visit_bool(bool) => Bool,
        visit_i8(i8) => I8,
        visit_i16(i16) => I16,
        visit_i32(i32) => I32,
        visit_i64(i64) => I64,
        visit_u8(u8) => U8,
        visit_u16(u16) => U16,
        visit_u32(u32) => U32,
        visit_u64(u64) => U64,
        visit_f32(f32) => F32,
        visit_f64(f64) => F64,
        visit_char(char) => Char,
    }

    fn visit_str<E>(self, value: &str) -> Result<Buffered, E> {
        Ok(Buffered::Str(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Buffered, E> {
        Ok(Buffered::Str(value))
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Buffered, E> {
        Ok(Buffered::Bytes(value.to_vec()))
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Buffered, E> {
        Ok(Buffered::Bytes(value))
    }

    fn visit_unit<E>(self) -> Result<Buffered, E> {
        Ok(Buffered::Unit)
    }

    fn visit_none<E>(self) -> Result<Buffered, E> {
        Ok(Buffered::Absent)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Buffered, D::Error> {
        Ok(Buffered::Present(Box::new(Buffered::deserialize(
            deserializer,
        )?)))
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Buffered, D::Error> {
        deserializer.deserialize_any(Capture)
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Buffered, A::Error> {
        let (kind, payload): (ArrayKind, _) = data.variant()?;
        Ok(Buffered::Array(
            kind,
            payload.newtype_variant_seed(ArrayPayload)?,
        ))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Buffered, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(4096));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Buffered::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Buffered, A::Error> {
        let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0).min(4096));
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(Buffered::Map(entries))
    }
}

impl Buffered {
    /// `human_readable` is what the format the value came from reports.
    pub fn deserializer<E>(self, human_readable: bool) -> BufferedDeserializer<E> {
        BufferedDeserializer {
            value: self,
            human_readable,
            error: PhantomData,
        }
    }
}

impl<'de, E: de::Error> IntoDeserializer<'de, E> for Buffered {
    type Deserializer = BufferedDeserializer<E>;

    fn into_deserializer(self) -> BufferedDeserializer<E> {
        self.deserializer(true)
    }
}

pub struct BufferedDeserializer<E> {
    value: Buffered,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

struct Items<E> {
    iter: vec::IntoIter<Buffered>,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> SeqAccess<'de> for Items<E> {
    type Error = E;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, E> {
        match self.iter.next() {
            Some(item) => seed
                .deserialize(item.deserializer(self.human_readable))
                .map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct Entries<E> {
    iter: vec::IntoIter<(Buffered, Buffered)>,
    value: Option<Buffered>,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> MapAccess<'de> for Entries<E> {
    type Error = E;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, E> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(key.deserializer(self.human_readable))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, E> {
        let value = self
            .value
            .take()
            .ok_or_else(|| E::custom("value is missing"))?;
        seed.deserialize(value.deserializer(self.human_readable))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

fn elements(kind: ArrayKind, payload: &[u8]) -> Vec<Buffered> {
    match kind {
        ArrayKind::Byte => payload
            .iter()
            .map(|&byte| Buffered::I8(i8::from_be_bytes([byte])))
            .collect(),
        ArrayKind::Int => (payload.as_chunks().0.iter())
            .map(|&chunk| Buffered::I32(i32::from_be_bytes(chunk)))
            .collect(),
        ArrayKind::Long => (payload.as_chunks().0.iter())
            .map(|&chunk| Buffered::I64(i64::from_be_bytes(chunk)))
            .collect(),
    }
}

struct ArrayVariant<E> {
    kind: ArrayKind,
    payload: Vec<u8>,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> EnumAccess<'de> for ArrayVariant<E> {
    type Error = E;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), E> {
        let name = <&str as IntoDeserializer<'de, E>>::into_deserializer(self.kind.variant());
        Ok((seed.deserialize(name)?, self))
    }
}

impl<'de, E: de::Error> VariantAccess<'de> for ArrayVariant<E> {
    type Error = E;

    fn unit_variant(self) -> Result<(), E> {
        Err(E::invalid_type(
            Unexpected::NewtypeVariant,
            &"a unit variant",
        ))
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, E> {
        seed.deserialize(Buffered::Array(self.kind, self.payload).deserializer(self.human_readable))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, E> {
        Err(E::invalid_type(
            Unexpected::NewtypeVariant,
            &"a tuple variant",
        ))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, E> {
        Err(E::invalid_type(
            Unexpected::NewtypeVariant,
            &"a struct variant",
        ))
    }
}

struct EntryVariant<E> {
    name: Buffered,
    value: Buffered,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> EnumAccess<'de> for EntryVariant<E> {
    type Error = E;
    type Variant = EntryValue<E>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, EntryValue<E>), E> {
        let variant = seed.deserialize(self.name.deserializer(self.human_readable))?;
        let value = EntryValue {
            value: self.value,
            human_readable: self.human_readable,
            error: PhantomData,
        };
        Ok((variant, value))
    }
}

struct EntryValue<E> {
    value: Buffered,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> VariantAccess<'de> for EntryValue<E> {
    type Error = E;

    fn unit_variant(self) -> Result<(), E> {
        match self.value {
            Buffered::Unit | Buffered::Absent => Ok(()),
            _ => Err(E::invalid_type(
                Unexpected::NewtypeVariant,
                &"a unit variant",
            )),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, E> {
        seed.deserialize(self.value.deserializer(self.human_readable))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, E> {
        self.value
            .deserializer::<E>(self.human_readable)
            .deserialize_seq(visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, E> {
        self.value
            .deserializer::<E>(self.human_readable)
            .deserialize_any(visitor)
    }
}

macro_rules! unsigned_from_signed {
    ($($method:ident => $visit:ident($int:ty) from $variant:ident),* $(,)?) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
            match self.value {
                Buffered::$variant(value) if !self.human_readable => visitor.$visit(value as $int),
                _ => self.deserialize_any(visitor),
            }
        }
    )*};
}

impl<'de, E: de::Error> Deserializer<'de> for BufferedDeserializer<E> {
    type Error = E;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        let human_readable = self.human_readable;
        match self.value {
            Buffered::Unit => visitor.visit_unit(),
            Buffered::Absent => visitor.visit_none(),
            Buffered::Present(inner) => visitor.visit_some(inner.deserializer(human_readable)),
            Buffered::Bool(value) => visitor.visit_bool(value),
            Buffered::I8(value) => visitor.visit_i8(value),
            Buffered::I16(value) => visitor.visit_i16(value),
            Buffered::I32(value) => visitor.visit_i32(value),
            Buffered::I64(value) => visitor.visit_i64(value),
            Buffered::U8(value) => visitor.visit_u8(value),
            Buffered::U16(value) => visitor.visit_u16(value),
            Buffered::U32(value) => visitor.visit_u32(value),
            Buffered::U64(value) => visitor.visit_u64(value),
            Buffered::F32(value) => visitor.visit_f32(value),
            Buffered::F64(value) => visitor.visit_f64(value),
            Buffered::Char(value) => visitor.visit_char(value),
            Buffered::Str(value) => visitor.visit_string(value),
            Buffered::Bytes(value) => visitor.visit_byte_buf(value),
            Buffered::Array(kind, payload) => Buffered::Seq(elements(kind, &payload))
                .deserializer(human_readable)
                .deserialize_any(visitor),
            Buffered::Seq(items) => {
                let length = items.len();
                let mut access = Items {
                    iter: items.into_iter(),
                    human_readable,
                    error: PhantomData,
                };
                let value = visitor.visit_seq(&mut access)?;
                match access.iter.len() {
                    0 => Ok(value),
                    _ => Err(E::invalid_length(length, &"fewer elements in sequence")),
                }
            }
            Buffered::Map(entries) => {
                let length = entries.len();
                let mut access = Entries {
                    iter: entries.into_iter(),
                    value: None,
                    human_readable,
                    error: PhantomData,
                };
                let value = visitor.visit_map(&mut access)?;
                match access.iter.len() {
                    0 => Ok(value),
                    _ => Err(E::invalid_length(length, &"fewer elements in map")),
                }
            }
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        match self.value {
            Buffered::Unit | Buffered::Absent => visitor.visit_none(),
            Buffered::Present(inner) => visitor.visit_some(inner.deserializer(self.human_readable)),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, E> {
        let human_readable = self.human_readable;
        match self.value {
            Buffered::Array(kind, payload) if name == NBT_ARRAY_TAG => {
                visitor.visit_enum(ArrayVariant {
                    kind,
                    payload,
                    human_readable,
                    error: PhantomData,
                })
            }
            value => visitor.visit_newtype_struct(value.deserializer(human_readable)),
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, E> {
        let human_readable = self.human_readable;
        match self.value {
            Buffered::Str(name) => visitor.visit_enum(
                <String as IntoDeserializer<'de, E>>::into_deserializer(name),
            ),
            Buffered::Map(mut entries) if entries.len() == 1 => {
                let (name, value) = entries.remove(0);
                visitor.visit_enum(EntryVariant {
                    name,
                    value,
                    human_readable,
                    error: PhantomData,
                })
            }
            other => Err(E::custom(format_args!(
                "expected a name or a map of one entry, found {other:?}"
            ))),
        }
    }

    // NBT has no boolean; a flag is a byte, and any number reads as one.
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        if self.human_readable {
            return self.deserialize_any(visitor);
        }
        match self.value {
            Buffered::I8(value) => visitor.visit_bool(value != 0),
            Buffered::I16(value) => visitor.visit_bool(value != 0),
            Buffered::I32(value) => visitor.visit_bool(value != 0),
            Buffered::I64(value) => visitor.visit_bool(value != 0),
            _ => self.deserialize_any(visitor),
        }
    }

    // NBT has only signed numbers; a binary format's unsigned field reads the
    // bits of the signed tag it was written as.
    unsigned_from_signed! {
        deserialize_u8 => visit_u8(u8) from I8,
        deserialize_u16 => visit_u16(u16) from I16,
        deserialize_u32 => visit_u32(u32) from I32,
        deserialize_u64 => visit_u64(u64) from I64,
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        match self.value {
            Buffered::Bytes(value) | Buffered::Array(_, value) => visitor.visit_byte_buf(value),
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        visitor.visit_unit()
    }

    fn is_human_readable(&self) -> bool {
        self.human_readable
    }

    forward_to_deserialize_any! {
        i8 i16 i32 i64 i128 u128 f32 f64 char str string unit unit_struct seq
        tuple tuple_struct map struct identifier
    }
}

/// What reading the body of a dispatched value failed with. The body is read
/// with this one error type whatever the format, so each type's derive is
/// compiled once rather than once per format; `read` hands the message on as
/// the format's own error.
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Error(message.to_string())
    }
}

#[doc(hidden)]
pub enum Pick {
    Variant(&'static str),
    Unsupported,
    Unknown,
}

/// The data type's own derive, handed the body of one chosen variant as an
/// externally tagged enum.
#[doc(hidden)]
pub struct Chosen<'de> {
    variant: &'static str,
    body: Vec<(Buffered, Buffered)>,
    human_readable: bool,
    lifetime: PhantomData<&'de ()>,
}

impl<'de> Deserializer<'de> for Chosen<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::custom("a dispatched value is read as an enum"))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(self)
    }

    fn is_human_readable(&self) -> bool {
        self.human_readable
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

impl<'de> EnumAccess<'de> for Chosen<'de> {
    type Error = Error;
    type Variant = Body;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Body), Error> {
        let name = <&str as IntoDeserializer<'de, Error>>::into_deserializer(self.variant);
        let body = Body {
            entries: self.body,
            human_readable: self.human_readable,
        };
        Ok((seed.deserialize(name)?, body))
    }
}

#[doc(hidden)]
pub struct Body {
    entries: Vec<(Buffered, Buffered)>,
    human_readable: bool,
}

impl<'de> VariantAccess<'de> for Body {
    type Error = Error;

    // The game's unit codecs read no fields and ignore the rest of the map.
    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(Buffered::Map(self.entries).deserializer(self.human_readable))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::invalid_type(Unexpected::TupleVariant, &"a map"))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        Buffered::Map(self.entries)
            .deserializer(self.human_readable)
            .deserialize_struct("", fields, visitor)
    }
}

struct MapEntries;

impl<'de> Visitor<'de> for MapEntries {
    type Value = Vec<(Buffered, Buffered)>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0).min(4096));
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}

/// Reads the map whole, takes the type key wherever it stands, and hands the
/// rest to `derive` as the body of the variant `pick` chose for it.
#[doc(hidden)]
pub fn read<'de, D, T, F>(
    deserializer: D,
    key: &'static str,
    registry: &'static str,
    pick: fn(&str) -> Pick,
    derive: F,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: FnOnce(Chosen<'de>) -> Result<T, Error>,
{
    let human_readable = deserializer.is_human_readable();
    let mut entries = deserializer.deserialize_map(MapEntries)?;
    let position = entries
        .iter()
        .position(|(name, _)| matches!(name, Buffered::Str(name) if name == key))
        .ok_or_else(|| D::Error::missing_field(key))?;
    let Buffered::Str(text) = entries.remove(position).1 else {
        return Err(D::Error::invalid_type(
            Unexpected::Other("a value that is no text"),
            &"a registry entry name",
        ));
    };
    let location = ResourceLocationBuf::read(&text).map_err(D::Error::custom)?;
    match pick(location.as_str()) {
        Pick::Variant(variant) => derive(Chosen {
            variant,
            body: entries,
            human_readable,
            lifetime: PhantomData,
        })
        .map_err(D::Error::custom),
        Pick::Unsupported => Err(D::Error::custom(format_args!(
            "{location} is not supported by {registry}"
        ))),
        Pick::Unknown => Err(D::Error::custom(format_args!(
            "Unknown registry key in ResourceKey[minecraft:root / {registry}]: {location}"
        ))),
    }
}

fn refusal<E: ser::Error>() -> E {
    E::custom("the body of a dispatched value must write as a map")
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Tag {
    pub key: &'static str,
    pub location: &'static str,
    /// Writes the key after the body, where vanilla's codec puts it for a few types.
    pub last: bool,
}

impl Tag {
    fn begin<S: Serializer>(
        self,
        serializer: S,
        length: Option<usize>,
    ) -> Result<TaggedFields<S::SerializeMap>, S::Error> {
        let length = length
            .map(|length| {
                length
                    .checked_add(1)
                    .ok_or_else(|| <S::Error as ser::Error>::custom("the body has too many fields"))
            })
            .transpose()?;
        let mut map = serializer.serialize_map(length)?;
        if !self.last {
            map.serialize_entry(self.key, self.location)?;
        }
        Ok(TaggedFields { map, tag: self })
    }
}

/// Runs a data type's derive so that what it writes as an externally tagged
/// enum comes out as one map holding the type key and the body.
#[doc(hidden)]
pub struct Tagging<S> {
    serializer: S,
    tag: Tag,
}

impl<S> Tagging<S> {
    pub fn new(serializer: S, tag: Tag) -> Self {
        Tagging { serializer, tag }
    }
}

struct Payload<S> {
    serializer: S,
    tag: Tag,
}

#[doc(hidden)]
pub struct TaggedFields<M> {
    map: M,
    tag: Tag,
}

impl<M: SerializeMap> TaggedFields<M> {
    fn finish(mut self) -> Result<M::Ok, M::Error> {
        if self.tag.last {
            self.map.serialize_entry(self.tag.key, self.tag.location)?;
        }
        self.map.end()
    }
}

impl<M: SerializeMap> SerializeMap for TaggedFields<M> {
    type Ok = M::Ok;
    type Error = M::Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), M::Error> {
        self.map.serialize_key(key)
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), M::Error> {
        self.map.serialize_value(value)
    }

    fn end(self) -> Result<M::Ok, M::Error> {
        self.finish()
    }
}

impl<M: SerializeMap> SerializeStruct for TaggedFields<M> {
    type Ok = M::Ok;
    type Error = M::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), M::Error> {
        self.map.serialize_entry(key, value)
    }

    fn end(self) -> Result<M::Ok, M::Error> {
        self.finish()
    }
}

impl<M: SerializeMap> SerializeStructVariant for TaggedFields<M> {
    type Ok = M::Ok;
    type Error = M::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), M::Error> {
        self.map.serialize_entry(key, value)
    }

    fn end(self) -> Result<M::Ok, M::Error> {
        self.finish()
    }
}

macro_rules! refuse_scalars {
    () => {
        type SerializeSeq = Impossible<S::Ok, S::Error>;
        type SerializeTuple = Impossible<S::Ok, S::Error>;
        type SerializeTupleStruct = Impossible<S::Ok, S::Error>;
        type SerializeTupleVariant = Impossible<S::Ok, S::Error>;

        fn serialize_bool(self, _: bool) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_i8(self, _: i8) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_i16(self, _: i16) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_i32(self, _: i32) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_i64(self, _: i64) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_u8(self, _: u8) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_u16(self, _: u16) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_u32(self, _: u32) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_u64(self, _: u64) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_f32(self, _: f32) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_f64(self, _: f64) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_char(self, _: char) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_str(self, _: &str) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_bytes(self, _: &[u8]) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_none(self) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<S::Ok, S::Error> {
            Err(refusal())
        }

        fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, S::Error> {
            Err(refusal())
        }

        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, S::Error> {
            Err(refusal())
        }

        fn serialize_tuple_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleStruct, S::Error> {
            Err(refusal())
        }

        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant, S::Error> {
            Err(refusal())
        }

        fn is_human_readable(&self) -> bool {
            self.serializer.is_human_readable()
        }
    };
}

impl<S: Serializer> Serializer for Tagging<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeMap = Impossible<S::Ok, S::Error>;
    type SerializeStruct = Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = TaggedFields<S::SerializeMap>;

    refuse_scalars!();

    fn serialize_unit(self) -> Result<S::Ok, S::Error> {
        Err(refusal())
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<S::Ok, S::Error> {
        Err(refusal())
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<S::Ok, S::Error> {
        self.tag.begin(self.serializer, Some(0))?.finish()
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<S::Ok, S::Error> {
        Err(refusal())
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(Payload {
            serializer: self.serializer,
            tag: self.tag,
        })
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, S::Error> {
        Err(refusal())
    }

    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, S::Error> {
        Err(refusal())
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, S::Error> {
        self.tag.begin(self.serializer, Some(length))
    }
}

impl<S: Serializer> Serializer for Payload<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeMap = TaggedFields<S::SerializeMap>;
    type SerializeStruct = TaggedFields<S::SerializeMap>;
    type SerializeStructVariant = Impossible<S::Ok, S::Error>;

    refuse_scalars!();

    fn serialize_unit(self) -> Result<S::Ok, S::Error> {
        self.tag.begin(self.serializer, Some(0))?.finish()
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<S::Ok, S::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<S::Ok, S::Error> {
        Err(refusal())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<S::Ok, S::Error> {
        Err(refusal())
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, S::Error> {
        self.tag.begin(self.serializer, length)
    }

    fn serialize_struct(
        self,
        _: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, S::Error> {
        self.tag.begin(self.serializer, Some(length))
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, S::Error> {
        Err(refusal())
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_nbt::{
        from_bytes, from_tag, nbt_int_array, nbt_long_array, to_bytes, to_nbt_compound,
    };
    use mcrs_minecraft_resource::Registered;
    use mcrs_minecraft_resource::{RegistryKey, rl};
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    mcrs_minecraft_registries::static_registry! {
        pub enum Shape;
        Circle = "minecraft:circle",
        Square = "minecraft:square",
        Hexagon = "minecraft:hexagon",
        Blob = "minecraft:blob",
    }

    impl Registered for Shape {
        const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:shape"));
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Side {
        length: u32,
    }

    #[crate::dispatch(registry = Shape, unsupported(Blob))]
    #[derive(Debug, Clone, PartialEq)]
    #[serde(deny_unknown_fields)]
    enum Figure {
        Circle,
        Square(Side),
        Hexagon {
            edge: u32,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            label: Option<String>,
        },
        #[dispatch(id = "mcrs:wobbly")]
        Wobbly {
            jitter: f32,
        },
    }

    const UNKNOWN: &str = "Unknown registry key in ResourceKey[minecraft:root / minecraft:shape]";

    #[test]
    fn a_dispatched_value_reads_and_writes_through_its_generated_enum() {
        let square = Figure::Square(Side { length: 3 });
        let hexagon = Figure::Hexagon {
            edge: 6,
            label: Some("h".into()),
        };
        let wobbly = Figure::Wobbly { jitter: 0.5 };
        let cases: [(&str, Result<Figure, &str>); 12] = [
            (r#"{"type":"minecraft:circle"}"#, Ok(Figure::Circle)),
            (r#"{"type":"circle"}"#, Ok(Figure::Circle)),
            (
                r#"{"type":"minecraft:circle","radius":1}"#,
                Ok(Figure::Circle),
            ),
            (
                r#"{"length":3,"type":"minecraft:square"}"#,
                Ok(square.clone()),
            ),
            (
                r#"{"edge":6,"type":"hexagon","label":"h"}"#,
                Ok(hexagon.clone()),
            ),
            (r#"{"type":"mcrs:wobbly","jitter":0.5}"#, Ok(wobbly.clone())),
            (
                r#"{"type":"minecraft:blob"}"#,
                Err("minecraft:blob is not supported"),
            ),
            (
                r#"{"type":"minecraft:cone"}"#,
                Err(
                    "Unknown registry key in ResourceKey[minecraft:root / minecraft:shape]: minecraft:cone",
                ),
            ),
            (
                r#"{"type":"mcrs:cone"}"#,
                Err(
                    "Unknown registry key in ResourceKey[minecraft:root / minecraft:shape]: mcrs:cone",
                ),
            ),
            (r#"{"type":"minecraft:wobbly","jitter":0.5}"#, Err(UNKNOWN)),
            (r#"{"edge":6}"#, Err("missing field `type`")),
            (
                r#"{"type":"minecraft:hexagon","edge":6,"sides":6}"#,
                Err("unknown field `sides`"),
            ),
        ];
        for (json, expected) in cases {
            let read = serde_json::from_str::<Figure>(json).map_err(|error| error.to_string());
            match expected {
                Ok(value) => assert_eq!(read, Ok(value), "{json}"),
                Err(text) => {
                    let error = read.expect_err(json);
                    assert!(error.contains(text), "{json}: {error}");
                }
            }
        }

        for value in [Figure::Circle, square, hexagon, wobbly] {
            let json = serde_json::to_string(&value).unwrap();
            assert!(json.starts_with(r#"{"type":""#), "{json}");
            assert_eq!(serde_json::from_str::<Figure>(&json).unwrap(), value);
        }
        assert_eq!(
            serde_json::to_string(&Figure::Circle).unwrap(),
            r#"{"type":"minecraft:circle"}"#
        );
        assert_eq!(Wide::Square.kind(), Shape::Square);
        assert_eq!(Wide::KINDS, [Shape::Circle, Shape::Square, Shape::Hexagon]);
    }

    #[crate::dispatch(registry = Shape, unsupported(Blob))]
    #[derive(Debug, Clone, PartialEq)]
    enum Wide {
        Circle {
            byte: i8,
            short: i16,
            int: i32,
            long: i64,
            float: f32,
            double: f64,
            text: String,
            #[serde(serialize_with = "nbt_int_array")]
            ints: Vec<i32>,
            #[serde(serialize_with = "nbt_long_array")]
            longs: Vec<i64>,
            tags: NbtCompound,
        },
        Square,
        Hexagon,
    }

    #[test]
    fn a_dispatched_body_keeps_its_nbt_tag_types() {
        let mut tags = NbtCompound::new();
        tags.put_byte("byte", -3);
        tags.put_short("short", 300);
        tags.put_int("int", 70_000);
        tags.put_long("long", 5_000_000_000);
        tags.put_float("float", 1.5);
        tags.put_double("double", 2.25);
        tags.put_string("string", "text".into());
        tags.put(
            "bytes",
            NbtTag::ByteArray(vec![1, 200, 3].into_boxed_slice()),
        );
        tags.put("ints", NbtTag::IntArray(vec![1, -2, 70_000]));
        tags.put("longs", NbtTag::LongArray(vec![1, -2, 5_000_000_000]));
        tags.put_list("list", vec![NbtTag::Int(1), NbtTag::Int(2)]);
        let mut inner = NbtCompound::new();
        inner.put_short("inner", 4);
        tags.put_component("compound", inner);

        let value = Wide::Circle {
            byte: -3,
            short: 300,
            int: 70_000,
            long: 5_000_000_000,
            float: 1.5,
            double: 2.25,
            text: "text".into(),
            ints: vec![1, -2, 70_000],
            longs: vec![1, -2, 5_000_000_000],
            tags,
        };
        let written = to_nbt_compound(&value).unwrap();
        assert_eq!(written.get_string("type"), Some("minecraft:circle"));
        assert_eq!(written.get_int("int"), Some(70_000));

        let from_memory: Wide = from_tag(NbtTag::Compound(written.clone())).unwrap();
        assert_eq!(from_memory, value);
        assert_eq!(to_nbt_compound(&from_memory).unwrap(), written);

        let mut bytes = Vec::new();
        to_bytes(&value, &mut bytes).unwrap();
        let from_file: Wide = from_bytes(Cursor::new(&bytes[..])).unwrap();
        assert_eq!(from_file, value);
        assert_eq!(to_nbt_compound(&from_file).unwrap(), written);
    }
}
