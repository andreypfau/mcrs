use crate::id::Id;
use crate::registry::Registry;
use mcrs_minecraft_core::{RegistryKey, ResourceLocation};
use serde::de::{SeqAccess, Visitor, value};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

pub enum EntrySet<R> {
    Tag(ResourceLocation<Arc<str>>),
    One(Id<R>),
    List(Vec<Id<R>>),
}

impl<R> EntrySet<R> {
    pub fn entries(&self) -> &[Id<R>] {
        match self {
            EntrySet::Tag(_) => &[],
            EntrySet::One(entry) => std::slice::from_ref(entry),
            EntrySet::List(entries) => entries,
        }
    }
}

impl<R> Clone for EntrySet<R> {
    fn clone(&self) -> Self {
        match self {
            EntrySet::Tag(tag) => EntrySet::Tag(tag.clone()),
            EntrySet::One(entry) => EntrySet::One(*entry),
            EntrySet::List(entries) => EntrySet::List(entries.clone()),
        }
    }
}

impl<R> fmt::Debug for EntrySet<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntrySet::Tag(tag) => f.debug_tuple("Tag").field(tag).finish(),
            EntrySet::One(entry) => f.debug_tuple("One").field(entry).finish(),
            EntrySet::List(entries) => f.debug_tuple("List").field(entries).finish(),
        }
    }
}

impl<R> PartialEq for EntrySet<R> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EntrySet::Tag(a), EntrySet::Tag(b)) => a == b,
            (EntrySet::One(a), EntrySet::One(b)) => a == b,
            (EntrySet::List(a), EntrySet::List(b)) => a == b,
            _ => false,
        }
    }
}

impl<R: RegistryKey> Serialize for EntrySet<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            EntrySet::Tag(tag) => serializer.serialize_str(&format!("#{}", tag.as_str())),
            EntrySet::One(entry) => entry.serialize(serializer),
            EntrySet::List(entries) => {
                let mut seq = serializer.serialize_seq(Some(entries.len()))?;
                for entry in entries {
                    seq.serialize_element(entry)?;
                }
                seq.end()
            }
        }
    }
}

impl<'de, R: RegistryKey> Deserialize<'de> for EntrySet<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<R>(PhantomData<fn() -> R>);

        impl<'de, R: RegistryKey> Visitor<'de> for SetVisitor<R> {
            type Value = EntrySet<R>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a tag, an entry, or a list of entries")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                let Some(tag) = text.strip_prefix('#') else {
                    return Id::deserialize(value::StrDeserializer::new(text)).map(EntrySet::One);
                };
                let tag = ResourceLocation::read(tag).map_err(E::custom)?;
                let listed =
                    Registry::<R>::in_scope("EntrySet", |registry| registry.has_tag(tag.as_str()))
                        .map_err(E::custom)?;
                if listed {
                    Ok(EntrySet::Tag(tag))
                } else {
                    Err(E::custom(format_args!(
                        "Missing tag: '{tag}' in '{}'",
                        R::KEY
                    )))
                }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(EntrySet::List)
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::set::RegistrySet;
    use mcrs_minecraft_core::rl;

    struct Marker;

    impl RegistryKey for Marker {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_marker");
    }

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(text).unwrap()
    }

    fn set() -> RegistrySet {
        let markers = Registry::<Marker>::new(
            ["minecraft:a", "minecraft:b", "minecraft:c"].map(name),
            [name("minecraft:t")],
        )
        .unwrap();
        RegistrySet::new().with(markers).unwrap()
    }

    fn id(text: &str) -> Id<Marker> {
        set().scope(|| serde_json::from_str(&format!("\"{text}\"")).unwrap())
    }

    #[test]
    fn an_entry_set_keeps_its_written_shape() {
        let a = id("minecraft:a");
        let b = id("minecraft:b");
        set().scope(|| {
            let cases: [(&str, EntrySet<Marker>); 4] = [
                (r##""#minecraft:t""##, EntrySet::Tag(name("minecraft:t"))),
                (r#""minecraft:a""#, EntrySet::One(a)),
                (r#"["minecraft:a"]"#, EntrySet::List(vec![a])),
                (
                    r#"["minecraft:a","minecraft:b"]"#,
                    EntrySet::List(vec![a, b]),
                ),
            ];
            for (text, expected) in cases {
                let read: EntrySet<Marker> = serde_json::from_str(text).unwrap();
                assert_eq!(read, expected, "{text}");
                assert_eq!(serde_json::to_string(&read).unwrap(), text);
            }
        });
    }

    #[test]
    fn an_entry_set_checks_its_tag_and_entries() {
        set().scope(|| {
            for text in [
                r##""#minecraft:nowhere""##,
                r#""minecraft:nowhere""#,
                r#"["minecraft:a","minecraft:nowhere"]"#,
            ] {
                let message = serde_json::from_str::<EntrySet<Marker>>(text)
                    .unwrap_err()
                    .to_string();
                assert!(message.contains("minecraft:test_marker"), "{message}");
                assert!(message.contains("minecraft:nowhere"), "{message}");
            }
        });
        let message = serde_json::from_str::<EntrySet<Marker>>(r##""#minecraft:t""##)
            .unwrap_err()
            .to_string();
        assert!(message.contains("minecraft:test_marker"), "{message}");
    }
}
