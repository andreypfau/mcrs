use std::fmt;
use std::marker::PhantomData;

use crate::registry::Registry;
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
                let location = ResourceLocation::read(text).map_err(E::custom)?;
                if let Ok(Err(unknown)) = Registry::<T>::in_scope("Holder", |registry| {
                    registry.require(location.as_str())
                }) {
                    return Err(E::custom(unknown));
                }
                Ok(Holder::reference(location))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::set::RegistrySet;
    use mcrs_minecraft_core::rl;
    use std::sync::Arc;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Sound;

    impl RegistryKey for Sound {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_sound");
    }

    struct Item;

    impl RegistryKey for Item {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_item");
    }

    fn registry<R: RegistryKey>(name: &str) -> Registry<R> {
        let name = ResourceLocation::<Arc<str>>::parse(name).unwrap();
        Registry::new([name], std::iter::empty()).unwrap()
    }

    fn read(name: &str) -> Result<Holder<Sound>, serde_json::Error> {
        serde_json::from_str(&format!("\"{name}\""))
    }

    #[test]
    fn a_reference_into_the_wrong_registry_fails() {
        let set = RegistrySet::new()
            .with(registry::<Sound>("minecraft:ding"))
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

        let unchecked = Holder::reference(ResourceLocation::minecraft("stick"));
        assert_eq!(read("minecraft:stick").unwrap(), unchecked);
        RegistrySet::new().scope(|| assert_eq!(read("minecraft:stick").unwrap(), unchecked));
    }
}
