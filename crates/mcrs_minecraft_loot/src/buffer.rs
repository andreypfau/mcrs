use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_registry::Holder;
use serde::de::value::{MapDeserializer, SeqDeserializer, StringDeserializer};
use serde::de::{
    DeserializeOwned, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use serde::{Deserialize, Deserializer, forward_to_deserialize_any};

/// A value read once and kept in serde's data model, so a codec can look at a
/// map's keys before choosing how to read its values.
#[derive(Debug, Clone)]
pub(crate) enum Buffered {
    Unit,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(String),
    Seq(Vec<Buffered>),
    Map(Vec<(Buffered, Buffered)>),
}

impl<'de> Deserialize<'de> for Buffered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct BufferVisitor;

        impl<'de> Visitor<'de> for BufferVisitor {
            type Value = Buffered;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any value")
            }

            fn visit_unit<E>(self) -> Result<Buffered, E> {
                Ok(Buffered::Unit)
            }

            fn visit_none<E>(self) -> Result<Buffered, E> {
                Ok(Buffered::Unit)
            }

            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Buffered, D::Error> {
                Buffered::deserialize(d)
            }

            fn visit_bool<E>(self, v: bool) -> Result<Buffered, E> {
                Ok(Buffered::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Buffered, E> {
                Ok(Buffered::I64(v))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Buffered, E> {
                Ok(Buffered::U64(v))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Buffered, E> {
                Ok(Buffered::F64(v))
            }

            fn visit_str<E>(self, v: &str) -> Result<Buffered, E> {
                Ok(Buffered::Str(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Buffered, E> {
                Ok(Buffered::Str(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Buffered, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Buffered::Seq(items))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Buffered, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Buffered::Map(entries))
            }
        }

        d.deserialize_any(BufferVisitor)
    }
}

pub(crate) struct BufferedDeserializer<E> {
    value: Buffered,
    error: PhantomData<E>,
}

impl<'de, E: serde::de::Error> IntoDeserializer<'de, E> for Buffered {
    type Deserializer = BufferedDeserializer<E>;

    fn into_deserializer(self) -> BufferedDeserializer<E> {
        BufferedDeserializer {
            value: self,
            error: PhantomData,
        }
    }
}

impl<'de, E: serde::de::Error> Deserializer<'de> for BufferedDeserializer<E> {
    type Error = E;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        match self.value {
            Buffered::Unit => visitor.visit_unit(),
            Buffered::Bool(v) => visitor.visit_bool(v),
            Buffered::I64(v) => visitor.visit_i64(v),
            Buffered::U64(v) => visitor.visit_u64(v),
            Buffered::F64(v) => visitor.visit_f64(v),
            Buffered::Str(v) => visitor.visit_string(v),
            Buffered::Seq(items) => {
                let mut seq = SeqDeserializer::new(items.into_iter());
                let value = visitor.visit_seq(&mut seq)?;
                seq.end()?;
                Ok(value)
            }
            Buffered::Map(entries) => {
                let mut map = MapDeserializer::new(entries.into_iter());
                let value = visitor.visit_map(&mut map)?;
                map.end()?;
                Ok(value)
            }
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
        match self.value {
            Buffered::Unit => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, E> {
        match self.value {
            Buffered::Str(v) => visitor.visit_enum(StringDeserializer::<E>::new(v)),
            other => Err(E::custom(format_args!("expected a name, found {other:?}"))),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, E> {
        visitor.visit_newtype_struct(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

/// A map read whole, by key.
pub(crate) struct MapEntries(Vec<(String, Buffered)>);

impl MapEntries {
    pub(crate) fn read<'de, A: MapAccess<'de>>(mut map: A) -> Result<Self, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, Buffered>()? {
            entries.push(entry);
        }
        Ok(MapEntries(entries))
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.0.iter().any(|(name, _)| name == key)
    }

    pub(crate) fn take(&mut self, key: &str) -> Option<Buffered> {
        let index = self.0.iter().position(|(name, _)| name == key)?;
        Some(self.0.remove(index).1)
    }

    pub(crate) fn into_value<T: DeserializeOwned, E: serde::de::Error>(self) -> Result<T, E> {
        T::deserialize(MapDeserializer::new(self.0.into_iter()))
    }

    pub(crate) fn into_holder<V, E>(self) -> Result<Holder<V>, E>
    where
        V: RegistryValue + DeserializeOwned,
        E: serde::de::Error,
    {
        self.into_value()
    }
}

/// Reads one buffered value with a seed chosen after the rest of its map.
pub(crate) fn read_seeded<'de, S, E>(seed: S, value: Buffered) -> Result<S::Value, E>
where
    S: DeserializeSeed<'de>,
    E: serde::de::Error,
{
    seed.deserialize(value.into_deserializer())
}
