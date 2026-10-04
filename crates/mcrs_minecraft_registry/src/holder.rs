use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::{RegistryKey, ResourceKey, ResourceLocation};
use serde::de::{DeserializeOwned, MapAccess, Visitor, value};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A registry id, or the entry itself written inline.
#[derive(Clone, PartialEq, Debug)]
pub enum Holder<T: RegistryKey> {
    Reference(ResourceKey<T>),
    Direct(T),
}

impl<T: RegistryKey> Holder<T> {
    pub fn reference(location: ResourceLocation) -> Self {
        Holder::Reference(ResourceKey::from_location(location))
    }
}

impl<T: RegistryKey + Serialize> Serialize for Holder<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(value) => value.serialize(s),
        }
    }
}

impl<'de, T: RegistryKey + DeserializeOwned> Deserialize<'de> for Holder<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct HolderVisitor<T>(PhantomData<T>);

        impl<'de, T: RegistryKey + DeserializeOwned> Visitor<'de> for HolderVisitor<T> {
            type Value = Holder<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a {} id or an inline entry", T::KEY.path())
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                ResourceLocation::read(text)
                    .map(Holder::reference)
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::MapAccessDeserializer::new(map)).map(Holder::Direct)
            }
        }

        d.deserialize_any(HolderVisitor(PhantomData))
    }
}

/// A holder whose persistent form is the registry id only; the inline entry
/// exists on the wire alone.
#[derive(Clone, PartialEq, Debug)]
pub struct HolderWireOnly<T: RegistryKey>(pub Holder<T>);

impl<T: RegistryKey + Serialize> Serialize for HolderWireOnly<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(_) => Err(S::Error::custom(format_args!(
                "an inline {} entry has no persistent form",
                T::KEY.path()
            ))),
        }
    }
}

impl<'de, T: RegistryKey> Deserialize<'de> for HolderWireOnly<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceKey::deserialize(d).map(|key| HolderWireOnly(Holder::Reference(key)))
    }
}
