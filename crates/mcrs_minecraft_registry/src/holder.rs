use std::fmt;
use std::marker::PhantomData;

use crate::registry::Registry;
use crate::set::ScopeError;
use mcrs_minecraft_core::{RegistryKey, RegistryValue, ResourceKey, ResourceLocation};
use serde::de::{DeserializeOwned, MapAccess, Visitor, value};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A registry id, or the entry itself written inline.
pub enum Holder<V: RegistryValue> {
    Reference(ResourceKey<V::Registry>),
    Direct(V),
}

impl<V: RegistryValue + Clone> Clone for Holder<V> {
    fn clone(&self) -> Self {
        match self {
            Holder::Reference(key) => Holder::Reference(key.clone()),
            Holder::Direct(value) => Holder::Direct(value.clone()),
        }
    }
}

impl<V: RegistryValue + PartialEq> PartialEq for Holder<V> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Holder::Reference(a), Holder::Reference(b)) => a == b,
            (Holder::Direct(a), Holder::Direct(b)) => a == b,
            _ => false,
        }
    }
}

impl<V: RegistryValue + fmt::Debug> fmt::Debug for Holder<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Holder::Reference(key) => f.debug_tuple("Reference").field(key).finish(),
            Holder::Direct(value) => f.debug_tuple("Direct").field(value).finish(),
        }
    }
}

impl<V: RegistryValue> Holder<V> {
    pub fn reference(location: ResourceLocation) -> Self {
        Holder::Reference(ResourceKey::from_location(location))
    }
}

impl<V: RegistryValue + Serialize> Serialize for Holder<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(value) => value.serialize(s),
        }
    }
}

impl<'de, V: RegistryValue + DeserializeOwned> Deserialize<'de> for Holder<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct HolderVisitor<V>(PhantomData<V>);

        impl<'de, V: RegistryValue + DeserializeOwned> Visitor<'de> for HolderVisitor<V> {
            type Value = Holder<V>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a {} id or an inline entry", V::Registry::KEY.path())
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                let location = ResourceLocation::read(text).map_err(E::custom)?;
                match Registry::<V::Registry>::in_scope("Holder", |registry| {
                    registry.require(location.as_str())
                }) {
                    Ok(Err(unknown)) => Err(E::custom(unknown)),
                    Err(missing @ ScopeError::MissingRegistry { .. }) => Err(E::custom(missing)),
                    Ok(Ok(_)) | Err(ScopeError::NoScope { .. }) => Ok(Holder::reference(location)),
                }
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                V::deserialize(value::MapAccessDeserializer::new(map)).map(Holder::Direct)
            }
        }

        d.deserialize_any(HolderVisitor(PhantomData))
    }
}

/// A holder whose persistent form is the registry id only; the inline entry
/// exists on the wire alone.
pub struct HolderWireOnly<V: RegistryValue>(pub Holder<V>);

impl<V: RegistryValue + Clone> Clone for HolderWireOnly<V> {
    fn clone(&self) -> Self {
        HolderWireOnly(self.0.clone())
    }
}

impl<V: RegistryValue + PartialEq> PartialEq for HolderWireOnly<V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<V: RegistryValue + fmt::Debug> fmt::Debug for HolderWireOnly<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HolderWireOnly").field(&self.0).finish()
    }
}

impl<V: RegistryValue + Serialize> Serialize for HolderWireOnly<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Holder::Reference(key) => key.serialize(s),
            Holder::Direct(_) => Err(S::Error::custom(format_args!(
                "an inline {} entry has no persistent form",
                V::Registry::KEY.path()
            ))),
        }
    }
}

impl<'de, V: RegistryValue> Deserialize<'de> for HolderWireOnly<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceKey::deserialize(d).map(|key| HolderWireOnly(Holder::Reference(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::set::RegistrySet;
    use mcrs_minecraft_core::rl;
    use std::sync::Arc;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Sound;

    struct Item;

    impl RegistryKey for Item {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_item");
    }

    struct SoundRegistry;

    impl RegistryKey for SoundRegistry {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_sound");
    }

    impl RegistryValue for Sound {
        type Registry = SoundRegistry;
    }

    fn registry<R: RegistryKey>(name: &str) -> Registry<R> {
        let name = ResourceLocation::<Arc<str>>::parse(name).unwrap();
        Registry::new([name]).unwrap()
    }

    fn read(name: &str) -> Result<Holder<Sound>, serde_json::Error> {
        serde_json::from_str(&format!("\"{name}\""))
    }

    #[test]
    fn a_reference_into_the_wrong_registry_fails() {
        let set = RegistrySet::new()
            .with(registry::<SoundRegistry>("minecraft:ding"))
            .unwrap()
            .with(registry::<Item>("minecraft:stick"))
            .unwrap();

        set.scope(|| {
            assert_eq!(
                read("minecraft:ding").unwrap(),
                Holder::reference(ResourceLocation::minecraft("ding"))
            );
            let message = read("minecraft:stick").unwrap_err().to_string();
            assert!(message.contains("minecraft:test_sound"), "{message}");
            assert!(message.contains("minecraft:stick"), "{message}");
        });

        assert_eq!(
            read("minecraft:stick").unwrap(),
            Holder::reference(ResourceLocation::minecraft("stick"))
        );
        RegistrySet::new().scope(|| {
            let message = read("minecraft:stick").unwrap_err().to_string();
            assert!(message.contains("no such registry"), "{message}");
        });
    }
}
