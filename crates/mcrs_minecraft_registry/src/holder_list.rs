use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::RegistryValue;
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor, value};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::holder::Holder;
use crate::holder_set::HolderSet;
use crate::tags::TagId;

/// A tag of a registry, or entries of it each named by id or written inline.
pub enum HolderList<V: RegistryValue> {
    Named(TagId<V::Registry>),
    One(Box<Holder<V>>),
    List(Vec<Holder<V>>),
}

impl<V: RegistryValue + Clone> Clone for HolderList<V> {
    fn clone(&self) -> Self {
        match self {
            HolderList::Named(tag) => HolderList::Named(*tag),
            HolderList::One(holder) => HolderList::One(holder.clone()),
            HolderList::List(holders) => HolderList::List(holders.clone()),
        }
    }
}

impl<V: RegistryValue + PartialEq> PartialEq for HolderList<V> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HolderList::Named(a), HolderList::Named(b)) => a == b,
            (HolderList::One(a), HolderList::One(b)) => a == b,
            (HolderList::List(a), HolderList::List(b)) => a == b,
            _ => false,
        }
    }
}

impl<V: RegistryValue + fmt::Debug> fmt::Debug for HolderList<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolderList::Named(tag) => f.debug_tuple("Named").field(tag).finish(),
            HolderList::One(holder) => f.debug_tuple("One").field(holder).finish(),
            HolderList::List(holders) => f.debug_tuple("List").field(holders).finish(),
        }
    }
}

impl<V: RegistryValue + Serialize> Serialize for HolderList<V>
where
    V::Registry: 'static,
{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            HolderList::Named(tag) => HolderSet::<V::Registry>::Named(*tag).serialize(s),
            HolderList::One(holder) => holder.serialize(s),
            HolderList::List(holders) => {
                let mut seq = s.serialize_seq(Some(holders.len()))?;
                for holder in holders {
                    seq.serialize_element(holder)?;
                }
                seq.end()
            }
        }
    }
}

impl<'de, V: RegistryValue + DeserializeOwned> Deserialize<'de> for HolderList<V>
where
    V::Registry: 'static,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ListVisitor<V>(PhantomData<V>);

        impl<'de, V: RegistryValue + DeserializeOwned> Visitor<'de> for ListVisitor<V>
        where
            V::Registry: 'static,
        {
            type Value = HolderList<V>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a tag, an entry, or a list of entries")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if text.starts_with('#') {
                    match HolderSet::<V::Registry>::deserialize(value::StrDeserializer::new(text))?
                    {
                        HolderSet::Named(tag) => Ok(HolderList::Named(tag)),
                        _ => Err(E::custom(format_args!("Not a tag id: {text}"))),
                    }
                } else {
                    Holder::deserialize(value::StrDeserializer::new(text))
                        .map(|holder| HolderList::One(Box::new(holder)))
                }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut holders = Vec::new();
                while let Some(holder) = seq.next_element::<Holder<V>>()? {
                    holders.push(holder);
                }
                Ok(HolderList::List(holders))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Holder::deserialize(value::MapAccessDeserializer::new(map))
                    .map(|holder| HolderList::One(Box::new(holder)))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Holder::deserialize(value::I64Deserializer::new(v))
                    .map(|holder| HolderList::One(Box::new(holder)))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Holder::deserialize(value::U64Deserializer::new(v))
                    .map(|holder| HolderList::One(Box::new(holder)))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Holder::deserialize(value::F64Deserializer::new(v))
                    .map(|holder| HolderList::One(Box::new(holder)))
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Holder::deserialize(value::BoolDeserializer::new(v))
                    .map(|holder| HolderList::One(Box::new(holder)))
            }
        }

        d.deserialize_any(ListVisitor(PhantomData))
    }
}
