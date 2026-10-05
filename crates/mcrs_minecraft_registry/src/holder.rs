use std::fmt;
use std::marker::PhantomData;

use crate::bitset::DenseId;
use crate::holder_set::skipping_sets;
use crate::id::Id;
use mcrs_minecraft_core::{RegistryKey, RegistryValue};
use serde::de::{DeserializeOwned, MapAccess, Visitor, value};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An entry of the registry its value belongs to, or the entry itself written inline.
pub enum Holder<V: RegistryValue> {
    Reference(Id<V::Registry>),
    Direct(V),
}

impl<V: RegistryValue + Clone> Clone for Holder<V> {
    fn clone(&self) -> Self {
        match self {
            Holder::Reference(id) => Holder::Reference(*id),
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
            Holder::Reference(id) => f.debug_tuple("Reference").field(id).finish(),
            Holder::Direct(value) => f.debug_tuple("Direct").field(value).finish(),
        }
    }
}

impl<V: RegistryValue + Serialize> Serialize for Holder<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Holder::Reference(id) => id.serialize(s),
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
                if skipping_sets() {
                    return Ok(Holder::Reference(Id::from_raw(0)));
                }
                Id::deserialize(value::StrDeserializer::new(text)).map(Holder::Reference)
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
            Holder::Reference(id) => id.serialize(s),
            Holder::Direct(_) => Err(S::Error::custom(format_args!(
                "an inline {} entry has no persistent form",
                V::Registry::KEY.path()
            ))),
        }
    }
}

impl<'de, V: RegistryValue> Deserialize<'de> for HolderWireOnly<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Id::deserialize(d).map(|id| HolderWireOnly(Holder::Reference(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holder_set::skip_sets;
    use crate::registry::Registry;
    use crate::set::RegistrySet;
    use mcrs_minecraft_core::{ResourceLocation, rl};
    use std::sync::Arc;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sound {
        volume: i32,
    }

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

    fn registry<R: RegistryKey>(names: &[&str]) -> Registry<R> {
        Registry::new(
            names
                .iter()
                .map(|name| ResourceLocation::<Arc<str>>::read(name).unwrap()),
        )
        .unwrap()
    }

    fn sounds(names: &[&str]) -> RegistrySet {
        RegistrySet::new()
            .with(registry::<SoundRegistry>(names))
            .unwrap()
    }

    fn read(name: &str) -> Result<Holder<Sound>, serde_json::Error> {
        serde_json::from_str(&format!("\"{name}\""))
    }

    fn write(holder: &Holder<Sound>) -> String {
        serde_json::to_string(holder).unwrap()
    }

    #[test]
    fn a_reference_holds_its_registry_id() {
        let set = sounds(&["minecraft:a", "minecraft:b"]);

        set.scope(|| {
            let Holder::Reference(id) = read("minecraft:b").unwrap() else {
                panic!("a name reads as a reference");
            };
            assert_eq!(
                id,
                set.registry::<SoundRegistry>()
                    .unwrap()
                    .by_name("minecraft:b")
                    .unwrap()
            );
            assert_eq!(id.number(), 1);
            assert_eq!(write(&Holder::Reference(id)), "\"minecraft:b\"");
        });
        let message = read("minecraft:b").unwrap_err().to_string();
        assert!(message.contains("no registry scope"), "{message}");
    }

    #[test]
    fn a_reference_into_the_wrong_registry_fails() {
        let set = RegistrySet::new()
            .with(registry::<SoundRegistry>(&["minecraft:ding"]))
            .unwrap()
            .with(registry::<Item>(&["minecraft:stick"]))
            .unwrap();

        set.scope(|| {
            assert!(read("minecraft:ding").is_ok());
            let message = read("minecraft:stick").unwrap_err().to_string();
            assert!(message.contains("minecraft:test_sound"), "{message}");
            assert!(message.contains("minecraft:stick"), "{message}");
        });

        RegistrySet::new().scope(|| {
            let message = read("minecraft:stick").unwrap_err().to_string();
            assert!(message.contains("no such registry"), "{message}");
        });
    }

    #[test]
    fn references_at_a_registry_s_ends_round_trip() {
        let names: Vec<String> = (0..=u16::from(u8::MAX) + 1)
            .map(|n| format!("minecraft:n{n}"))
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let set = sounds(&refs);

        set.scope(|| {
            for (name, number) in [(&names[0], 0), (names.last().unwrap(), names.len() - 1)] {
                let holder = read(name).unwrap();
                let Holder::Reference(id) = holder else {
                    panic!("a name reads as a reference");
                };
                assert_eq!(usize::from(id.number()), number, "{name}");
                assert_eq!(&write(&Holder::Reference(id)), &format!("\"{name}\""));
            }
        });
    }

    #[test]
    fn a_walk_that_skips_sets_reads_a_reference_without_a_registry() {
        let skipped = skip_sets(|| read("minecraft:anything").unwrap());
        assert!(matches!(skipped, Holder::Reference(_)));
        assert!(read("minecraft:anything").is_err());
    }

    #[test]
    fn an_inline_holder_keeps_its_value_and_an_empty_reference_fails() {
        sounds(&["minecraft:a"]).scope(|| {
            let inline: Holder<Sound> = serde_json::from_str(r#"{"volume":3}"#).unwrap();
            assert_eq!(inline, Holder::Direct(Sound { volume: 3 }));
            assert_eq!(write(&inline), r#"{"volume":3}"#);

            let message = read("").unwrap_err().to_string();
            assert!(message.contains("minecraft:test_sound"), "{message}");
        });
    }
}
