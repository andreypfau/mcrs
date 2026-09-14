use std::fmt;
use std::marker::PhantomData;

use serde::de::Error as _;
use serde::de::{MapAccess, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ResourceLocation;

/// A registry element set as `RegistryCodecs.holderSet` writes it: a `#tag`,
/// one entry, or a list of entries. The three shapes are kept apart so a value
/// serializes back the way it came. With `ALWAYS_LIST` — the codec's
/// `alwaysUseList` — a bare entry is refused and only a tag or a list reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HolderSet<T = ResourceLocation, const ALWAYS_LIST: bool = false> {
    Tag(ResourceLocation),
    One(T),
    List(Vec<T>),
}

impl<T, const ALWAYS_LIST: bool> HolderSet<T, ALWAYS_LIST> {
    /// The entries a tag-less set names; a tag names none until it is expanded.
    pub fn entries(&self) -> &[T] {
        match self {
            HolderSet::Tag(_) => &[],
            HolderSet::One(entry) => std::slice::from_ref(entry),
            HolderSet::List(entries) => entries,
        }
    }
}

impl<'de, T: Deserialize<'de>, const ALWAYS_LIST: bool> Deserialize<'de>
    for HolderSet<T, ALWAYS_LIST>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<T, const ALWAYS_LIST: bool>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>, const ALWAYS_LIST: bool> Visitor<'de>
            for SetVisitor<T, ALWAYS_LIST>
        {
            type Value = HolderSet<T, ALWAYS_LIST>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(if ALWAYS_LIST {
                    "a tag or a list of entries"
                } else {
                    "a tag, an entry, or a list of entries"
                })
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if let Some(tag) = text.strip_prefix('#') {
                    return ResourceLocation::parse(tag)
                        .map(HolderSet::Tag)
                        .map_err(E::custom);
                }
                if ALWAYS_LIST {
                    return Err(E::custom(format!("Not a tag id: {text}")));
                }
                T::deserialize(value::StrDeserializer::new(text)).map(HolderSet::One)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                if ALWAYS_LIST {
                    return Err(A::Error::custom("Not a tag id: an inline entry"));
                }
                T::deserialize(value::MapAccessDeserializer::new(map)).map(HolderSet::One)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(HolderSet::List)
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

impl<T: Serialize, const ALWAYS_LIST: bool> Serialize for HolderSet<T, ALWAYS_LIST> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            HolderSet::Tag(tag) => serializer.serialize_str(&format!("#{}", tag.as_str())),
            HolderSet::One(entry) => entry.serialize(serializer),
            HolderSet::List(entries) => entries.serialize(serializer),
        }
    }
}
