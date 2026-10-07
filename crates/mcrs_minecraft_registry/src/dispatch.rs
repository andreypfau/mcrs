use std::fmt;
use std::marker::PhantomData;
use std::vec;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::{ArrayKind, NBT_ARRAY_NEWTYPE};
use serde::de::{
    self, DeserializeSeed, EnumAccess, Error as _, IntoDeserializer, MapAccess, SeqAccess,
    Unexpected, VariantAccess, Visitor,
};
use serde::ser::{self, Impossible, SerializeMap, SerializeStruct, SerializeStructVariant};
use serde::{Deserialize, Deserializer, Serialize, Serializer, forward_to_deserialize_any};

#[doc(hidden)]
pub use mcrs_minecraft_core as __core;
#[doc(hidden)]
pub use serde as __serde;

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
        deserializer.deserialize_newtype_struct(NBT_ARRAY_NEWTYPE, Capture)
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
    payload
        .chunks_exact(kind.width())
        .map(|chunk| match kind {
            ArrayKind::Byte => Buffered::I8(chunk[0] as i8),
            ArrayKind::Int => Buffered::I32(i32::from_be_bytes(chunk.try_into().unwrap())),
            ArrayKind::Long => Buffered::I64(i64::from_be_bytes(chunk.try_into().unwrap())),
        })
        .collect()
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
            Buffered::Array(kind, payload) if name == NBT_ARRAY_NEWTYPE => {
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

#[doc(hidden)]
pub enum Pick {
    Variant(&'static str),
    Unsupported,
    Unknown,
}

/// The data type's own derive, handed the body of one chosen variant as an
/// externally tagged enum.
#[doc(hidden)]
pub struct Chosen<'de, E> {
    variant: &'static str,
    body: Vec<(Buffered, Buffered)>,
    human_readable: bool,
    lifetime: PhantomData<&'de ()>,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> Deserializer<'de> for Chosen<'de, E> {
    type Error = E;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, E> {
        Err(E::custom("a dispatched value is read as an enum"))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, E> {
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

impl<'de, E: de::Error> EnumAccess<'de> for Chosen<'de, E> {
    type Error = E;
    type Variant = Body<E>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Body<E>), E> {
        let name = <&str as IntoDeserializer<'de, E>>::into_deserializer(self.variant);
        let body = Body {
            entries: self.body,
            human_readable: self.human_readable,
            error: PhantomData,
        };
        Ok((seed.deserialize(name)?, body))
    }
}

#[doc(hidden)]
pub struct Body<E> {
    entries: Vec<(Buffered, Buffered)>,
    human_readable: bool,
    error: PhantomData<fn() -> E>,
}

impl<'de, E: de::Error> VariantAccess<'de> for Body<E> {
    type Error = E;

    // The game's unit codecs read no fields and ignore the rest of the map.
    fn unit_variant(self) -> Result<(), E> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, E> {
        seed.deserialize(Buffered::Map(self.entries).deserializer(self.human_readable))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, E> {
        Err(E::invalid_type(Unexpected::TupleVariant, &"a map"))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, E> {
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
    F: FnOnce(Chosen<'de, D::Error>) -> Result<T, D::Error>,
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
    let location = ResourceLocation::read(&text).map_err(D::Error::custom)?;
    match pick(location.as_str()) {
        Pick::Variant(variant) => derive(Chosen {
            variant,
            body: entries,
            human_readable,
            lifetime: PhantomData,
            error: PhantomData,
        }),
        Pick::Unsupported => Err(D::Error::custom(format_args!(
            "{location} is not supported by {registry}"
        ))),
        Pick::Unknown => Err(D::Error::custom(crate::static_key::unknown_entry(
            registry, &location,
        ))),
    }
}

fn refusal<E: ser::Error>() -> E {
    E::custom("the body of a dispatched value must write as a map")
}

fn begin<S: Serializer>(
    serializer: S,
    key: &'static str,
    location: &'static str,
    length: Option<usize>,
) -> Result<S::SerializeMap, S::Error> {
    let mut map = serializer.serialize_map(length.map(|length| length + 1))?;
    map.serialize_entry(key, location)?;
    Ok(map)
}

/// Runs a data type's derive so that what it writes as an externally tagged
/// enum comes out as one map with the type key first and the body after it.
#[doc(hidden)]
pub struct Tagging<S> {
    serializer: S,
    key: &'static str,
    location: &'static str,
}

impl<S> Tagging<S> {
    pub fn new(serializer: S, key: &'static str, location: &'static str) -> Self {
        Tagging {
            serializer,
            key,
            location,
        }
    }
}

struct Payload<S> {
    serializer: S,
    key: &'static str,
    location: &'static str,
}

#[doc(hidden)]
pub struct TaggedFields<M>(M);

impl<M: SerializeMap> SerializeStruct for TaggedFields<M> {
    type Ok = M::Ok;
    type Error = M::Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), M::Error> {
        self.0.serialize_entry(key, value)
    }

    fn end(self) -> Result<M::Ok, M::Error> {
        self.0.end()
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
        self.0.serialize_entry(key, value)
    }

    fn end(self) -> Result<M::Ok, M::Error> {
        self.0.end()
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
        begin(self.serializer, self.key, self.location, Some(0))?.end()
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
            key: self.key,
            location: self.location,
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
        begin(self.serializer, self.key, self.location, Some(length)).map(TaggedFields)
    }
}

impl<S: Serializer> Serializer for Payload<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = TaggedFields<S::SerializeMap>;
    type SerializeStructVariant = Impossible<S::Ok, S::Error>;

    refuse_scalars!();

    fn serialize_unit(self) -> Result<S::Ok, S::Error> {
        begin(self.serializer, self.key, self.location, Some(0))?.end()
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
        begin(self.serializer, self.key, self.location, length)
    }

    fn serialize_struct(
        self,
        _: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, S::Error> {
        begin(self.serializer, self.key, self.location, Some(length)).map(TaggedFields)
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

/// Reads and writes a data enum through the generated enum of the static
/// registry its type key names.
///
/// The data type is derived with `#[serde(remote = "Self")]` and no tag or
/// renames; this implements the real `Serialize` and `Deserialize` around the
/// derive. Both directions match exhaustively, over the generated enum when
/// reading and over the data type when writing, so an entry or a variant with
/// no arm does not compile. `unsupported` lists entries the data type does not
/// model, `extend` adds names outside the registry, and `wildcard unsupported`
/// refuses every entry not listed, for a deliberate subset. A leading
/// `reads_only` leaves `Serialize` to the type, for one that writes its type
/// key somewhere else than first. A leading
/// `validated` runs the type's `Validate` after the read, and a leading
/// `for<P> serialize { .. } deserialize { .. }` carries the generics and the
/// bounds of the two impls.
#[macro_export]
macro_rules! dispatch {
    (@check true $value:ident) => {
        $crate::dispatch::__core::codec::Validate::validate(&$value)
            .map_err(<D::Error as $crate::dispatch::__serde::de::Error>::custom)?;
    };
    (@check false $value:ident) => {};
    (@kind [$($gp:ident),*] $ty:ty, $gen:ty, [$($g:ident => $d:ident),+], []) => {
        impl<$($gp),*> $ty {
            #[allow(dead_code)]
            pub const KINDS: &[$gen] = &[$(<$gen>::$g),+];

            #[allow(dead_code)]
            pub fn kind(&self) -> $gen {
                match self {
                    $( Self::$d { .. } => <$gen>::$g, )+
                }
            }
        }
    };
    (@kind [$($gp:ident),*] $ty:ty, $gen:ty, [$($g:ident => $d:ident),+], [$($ext:literal)+]) => {};
    (@write true { $($item:item)* }) => { $($item)* };
    (@write false { $($item:item)* }) => {};
    (@build [$validated:tt $write:tt] [$($gp:ident),*] [$($sb:tt)*] [$($db:tt)*] $ty:ty,
        key = $key:literal, registry = $gen:ty,
        { $($g:ident => $d:ident),+ $(,)? }
        $(unsupported { $($u:ident),* $(,)? })?
        $(extend { $($ext:literal => $ed:ident),+ $(,)? })?
        $(wildcard $wild:ident)?
    ) => {
        impl<'de $(, $gp)*> $crate::dispatch::__serde::Deserialize<'de> for $ty
        where
            $($db)*
        {
            fn deserialize<D: $crate::dispatch::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                #[allow(clippy::match_single_binding)]
                fn pick(location: &str) -> $crate::dispatch::Pick {
                    use $crate::dispatch::Pick;
                    match <$gen>::find(location) {
                        ::core::option::Option::Some(entry) => match entry {
                            $( <$gen>::$g => Pick::Variant(stringify!($d)), )+
                            $($( <$gen>::$u => Pick::Unsupported, )*)?
                            $( _ => {
                                let _ = stringify!($wild);
                                Pick::Unsupported
                            } )?
                        },
                        ::core::option::Option::None => match location {
                            $($( $ext => Pick::Variant(stringify!($ed)), )+)?
                            _ => Pick::Unknown,
                        },
                    }
                }

                let value = $crate::dispatch::read(
                    deserializer,
                    $key,
                    <$gen as $crate::Registered>::REGISTRY.location().as_static_str(),
                    pick,
                    |body| <$ty>::deserialize(body),
                )?;
                $crate::dispatch!(@check $validated value);
                ::core::result::Result::Ok(value)
            }
        }

        $crate::dispatch!(@write $write {
        impl<$($gp),*> $crate::dispatch::__serde::Serialize for $ty
        where
            $($sb)*
        {
            fn serialize<S: $crate::dispatch::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                let location: &'static str = match self {
                    $( Self::$d { .. } => <$gen>::$g.as_static_str(), )+
                    $($( Self::$ed { .. } => $ext, )+)?
                };
                <$ty>::serialize(
                    self,
                    $crate::dispatch::Tagging::new(serializer, $key, location),
                )
            }
        }
        });

        $crate::dispatch!(@kind [$($gp),*] $ty, $gen, [$($g => $d),+], [$($($ext)+)?]);
    };
    (for<$($gp:ident),+> serialize { $($sb:tt)* } deserialize { $($db:tt)* } $ty:ty, $($rest:tt)+) => {
        $crate::dispatch!(@build [false true] [$($gp),+] [$($sb)*] [$($db)*] $ty, $($rest)+);
    };
    (validated $ty:ty, $($rest:tt)+) => {
        $crate::dispatch!(@build [true true] [] [] [] $ty, $($rest)+);
    };
    (reads_only $ty:ty, $($rest:tt)+) => {
        $crate::dispatch!(@build [false false] [] [] [] $ty, $($rest)+);
    };
    ($ty:ty, $($rest:tt)+) => {
        $crate::dispatch!(@build [false true] [] [] [] $ty, $($rest)+);
    };
}

/// A map whose key selects both the field and the type of its value, the way
/// `Codec.dispatchedMap` does. The key is read as the generated enum of the
/// registry, so an entry with no arm does not compile; `unsupported` lists the
/// entries the type does not model, which a pack must not state.
#[macro_export]
macro_rules! dispatched_map {
    (
        $(#[$meta:meta])*
        $name:ident on $gen:ty {
            $($g:ident => $field:ident : $ty:ty),+ $(,)?
        }
        $(unsupported { $($u:ident),* $(,)? })?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct $name {
            $(pub $field: Option<$ty>,)+
        }

        impl $name {
            pub fn is_empty(&self) -> bool {
                true $(&& self.$field.is_none())+
            }
        }

        impl<'de> $crate::dispatch::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::dispatch::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                struct V;

                impl<'de> $crate::dispatch::__serde::de::Visitor<'de> for V {
                    type Value = $name;

                    fn expecting(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " map"))
                    }

                    fn visit_map<A: $crate::dispatch::__serde::de::MapAccess<'de>>(
                        self,
                        mut map: A,
                    ) -> ::core::result::Result<$name, A::Error> {
                        use $crate::dispatch::__serde::de::Error as _;
                        let mut out = <$name>::default();
                        while let ::core::option::Option::Some(key) = map.next_key::<String>()? {
                            let entry = <$gen as $crate::dispatch::__serde::Deserialize>::deserialize(
                                $crate::dispatch::__serde::de::value::StrDeserializer::<A::Error>::new(&key),
                            )?;
                            match entry {
                                $( <$gen>::$g => {
                                    if out.$field.is_some() {
                                        return Err(A::Error::custom(format_args!(
                                            "Duplicate key '{}'",
                                            entry.as_static_str()
                                        )));
                                    }
                                    out.$field = Some(map.next_value().map_err(|e| {
                                        A::Error::custom(format_args!("{}: {e}", entry.as_static_str()))
                                    })?);
                                } )+
                                $($( <$gen>::$u => {
                                    return Err(A::Error::custom(format_args!(
                                        "{} is not supported by {}",
                                        entry.as_static_str(),
                                        <$gen as $crate::Registered>::REGISTRY.location()
                                    )));
                                } )*)?
                            }
                        }
                        Ok(out)
                    }
                }

                deserializer.deserialize_map(V)
            }
        }

        impl $crate::dispatch::__serde::Serialize for $name {
            fn serialize<S: $crate::dispatch::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                use $crate::dispatch::__serde::ser::SerializeMap as _;
                let length = 0usize $(+ self.$field.is_some() as usize)+;
                let mut map = serializer.serialize_map(Some(length))?;
                $(if let Some(value) = &self.$field {
                    map.serialize_entry(<$gen>::$g.as_static_str(), value)?;
                })+
                map.end()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::Registered;
    use mcrs_minecraft_core::{RegistryKey, rl};
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_nbt::{
        from_bytes, from_tag, nbt_int_array, nbt_long_array, to_bytes, to_nbt_compound,
    };
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    crate::static_registry! {
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

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(remote = "Self", deny_unknown_fields)]
    enum Figure {
        Circle,
        Square(Side),
        Hexagon {
            edge: u32,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            label: Option<String>,
        },
        Wobbly {
            jitter: f32,
        },
    }

    crate::dispatch! {
        Figure, key = "type", registry = Shape,
        {
            Circle => Circle,
            Square => Square,
            Hexagon => Hexagon,
        }
        unsupported { Blob }
        extend { "mcrs:wobbly" => Wobbly }
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
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(remote = "Self")]
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

    crate::dispatch! {
        Wide, key = "type", registry = Shape,
        {
            Circle => Circle,
            Square => Square,
            Hexagon => Hexagon,
        }
        unsupported { Blob }
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
