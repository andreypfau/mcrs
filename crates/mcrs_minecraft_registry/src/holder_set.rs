use crate::id::Id;
use crate::tags::{TagId, Tags};
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::{RegistryKey, ResourceLocation};
use serde::de::{SeqAccess, Visitor, value};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

thread_local! {
    static WALKING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `run` with holder sets reading as empty and holder references as the
/// first entry, without consulting any registry, for a caller that needs only
/// a value's extent or its fields that name no registry entry, and drops every
/// set and reference it reads.
pub fn skip_sets<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);

    impl Drop for Restore {
        fn drop(&mut self) {
            WALKING.set(self.0);
        }
    }

    let _restore = Restore(WALKING.replace(true));
    run()
}

pub fn skipping_sets() -> bool {
    WALKING.get()
}

pub enum HolderSet<R, const ALWAYS_LIST: bool = false> {
    Named(TagId<R>),
    One(Id<R>),
    List(Box<[Id<R>]>),
}

impl<R, const ALWAYS_LIST: bool> HolderSet<R, ALWAYS_LIST> {
    pub fn tag(&self) -> Option<TagId<R>> {
        match self {
            HolderSet::Named(tag) => Some(*tag),
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<R>, tags: &Tags<R>) -> bool {
        match self {
            HolderSet::Named(tag) => tags.contains(*tag, id),
            HolderSet::One(entry) => *entry == id,
            HolderSet::List(entries) => entries.contains(&id),
        }
    }

    pub fn ids<'a>(&'a self, tags: &'a Tags<R>) -> impl Iterator<Item = Id<R>> + 'a
    where
        R: 'a,
    {
        let (direct, named) = match self {
            HolderSet::Named(tag) => (&[][..], Some(*tag)),
            HolderSet::One(entry) => (std::slice::from_ref(entry), None),
            HolderSet::List(entries) => (&entries[..], None),
        };
        direct
            .iter()
            .copied()
            .chain(named.into_iter().flat_map(move |tag| tags.members(tag)))
    }
}

impl<R, const ALWAYS_LIST: bool> Default for HolderSet<R, ALWAYS_LIST> {
    fn default() -> Self {
        HolderSet::List(Box::new([]))
    }
}

impl<R, const ALWAYS_LIST: bool> Clone for HolderSet<R, ALWAYS_LIST> {
    fn clone(&self) -> Self {
        match self {
            HolderSet::Named(tag) => HolderSet::Named(*tag),
            HolderSet::One(entry) => HolderSet::One(*entry),
            HolderSet::List(entries) => HolderSet::List(entries.clone()),
        }
    }
}

impl<R, const ALWAYS_LIST: bool> fmt::Debug for HolderSet<R, ALWAYS_LIST> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolderSet::Named(tag) => f.debug_tuple("Named").field(tag).finish(),
            HolderSet::One(entry) => f.debug_tuple("One").field(entry).finish(),
            HolderSet::List(entries) => f.debug_tuple("List").field(entries).finish(),
        }
    }
}

impl<R, const ALWAYS_LIST: bool> PartialEq for HolderSet<R, ALWAYS_LIST> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HolderSet::Named(a), HolderSet::Named(b)) => a == b,
            (HolderSet::One(a), HolderSet::One(b)) => a == b,
            (HolderSet::List(a), HolderSet::List(b)) => a == b,
            (HolderSet::One(one), HolderSet::List(list))
            | (HolderSet::List(list), HolderSet::One(one)) => **list == [*one],
            _ => false,
        }
    }
}

impl<R, const ALWAYS_LIST: bool> Eq for HolderSet<R, ALWAYS_LIST> {}

impl<R, const ALWAYS_LIST: bool> Hash for HolderSet<R, ALWAYS_LIST> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            HolderSet::Named(tag) => tag.hash(state),
            HolderSet::One(entry) => std::slice::from_ref(entry).hash(state),
            HolderSet::List(entries) => entries.hash(state),
        }
    }
}

impl<R: RegistryKey, const ALWAYS_LIST: bool> Serialize for HolderSet<R, ALWAYS_LIST> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            HolderSet::Named(tag) => {
                let name = Tags::<R>::in_scope("HolderSet", |tags| tags.name(*tag).clone())
                    .map_err(S::Error::custom)?;
                serializer.serialize_str(&format!("#{}", name.as_str()))
            }
            HolderSet::One(entry) if !ALWAYS_LIST => entry.serialize(serializer),
            HolderSet::One(entry) => {
                let mut seq = serializer.serialize_seq(Some(1))?;
                seq.serialize_element(entry)?;
                seq.end()
            }
            HolderSet::List(entries) => {
                let mut seq = serializer.serialize_seq(Some(entries.len()))?;
                for entry in entries {
                    seq.serialize_element(entry)?;
                }
                seq.end()
            }
        }
    }
}

impl<'de, R: RegistryKey, const ALWAYS_LIST: bool> Deserialize<'de> for HolderSet<R, ALWAYS_LIST> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<R, const ALWAYS_LIST: bool>(PhantomData<fn() -> R>);

        impl<'de, R: RegistryKey, const ALWAYS_LIST: bool> Visitor<'de> for SetVisitor<R, ALWAYS_LIST> {
            type Value = HolderSet<R, ALWAYS_LIST>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(if ALWAYS_LIST {
                    "a tag or a list of entries"
                } else {
                    "a tag, an entry, or a list of entries"
                })
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if skipping_sets() {
                    return Ok(HolderSet::default());
                }
                let Some(tag) = text.strip_prefix('#') else {
                    if ALWAYS_LIST {
                        return Err(E::custom(format_args!("Not a tag id: {text}")));
                    }
                    return Id::deserialize(value::StrDeserializer::new(text)).map(HolderSet::One);
                };
                let key =
                    TagKey::<R, _>::from_location(ResourceLocation::read(tag).map_err(E::custom)?);
                Tags::<R>::in_scope("HolderSet", |tags| tags.get(&key))
                    .map_err(E::custom)?
                    .map(HolderSet::Named)
                    .ok_or_else(|| {
                        E::custom(format_args!(
                            "Missing tag: '{}' in '{}'",
                            key.as_str(),
                            R::KEY
                        ))
                    })
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                if skipping_sets() {
                    while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                    return Ok(HolderSet::default());
                }
                Vec::<Id<R>>::deserialize(value::SeqAccessDeserializer::new(seq))
                    .map(|entries| HolderSet::List(entries.into_boxed_slice()))
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use crate::set::RegistrySet;
    use crate::tags::{TagRules, TagSource, TagTable, build_tags};
    use mcrs_minecraft_core::rl;
    use std::sync::Arc;

    struct Marker;

    impl RegistryKey for Marker {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_marker");
    }

    type Name = ResourceLocation<Arc<str>>;
    type Set = HolderSet<Marker>;
    type Listed = HolderSet<Marker, true>;

    fn name(text: &str) -> Name {
        ResourceLocation::parse(text).unwrap()
    }

    fn markers() -> Registry<Marker> {
        Registry::new(["minecraft:a", "minecraft:b", "minecraft:c", "minecraft:d"].map(name))
            .unwrap()
    }

    fn table(prior: Option<&[Name]>, tags: &[(&str, &str)]) -> Arc<TagTable> {
        let files: Vec<(Name, Vec<TagSource<'_>>)> = tags
            .iter()
            .map(|(tag, json)| {
                let source = TagSource {
                    pack: "test",
                    path: "minecraft/tags/test_marker/file.json",
                    bytes: json.as_bytes(),
                };
                (name(tag), vec![source])
            })
            .collect();
        let (table, problems) = build_tags(markers().table(), TagRules::World, &files, prior);
        assert!(problems.is_empty(), "{problems:?}");
        Arc::new(table)
    }

    const TAGS: [(&str, &str); 3] = [
        ("minecraft:t", r#"{"values":["minecraft:c","minecraft:a"]}"#),
        ("minecraft:x", r#"{"values":["minecraft:b"]}"#),
        ("minecraft:none", r#"{"values":[]}"#),
    ];

    fn set() -> RegistrySet {
        RegistrySet::new()
            .with(markers())
            .unwrap()
            .with_tags(table(None, &TAGS))
    }

    fn id(text: &str) -> Id<Marker> {
        markers().get(text).unwrap()
    }

    fn tag(tags: &Tags<Marker>, text: &str) -> TagId<Marker> {
        tags.get(&TagKey::<Marker, _>::from_location(name(text)))
            .unwrap_or_else(|| panic!("{text} is a tag"))
    }

    fn names(ids: impl Iterator<Item = Id<Marker>>) -> Vec<&'static str> {
        ids.map(|id| ["a", "b", "c", "d"][id.index()]).collect()
    }

    #[test]
    fn a_holder_set_keeps_its_written_shape() {
        let set = set();
        let tags = set.tags::<Marker>().unwrap();
        let (a, b) = (id("minecraft:a"), id("minecraft:b"));
        set.scope(|| {
            let cases: [(&str, Set); 5] = [
                (r##""#minecraft:t""##, Set::Named(tag(&tags, "minecraft:t"))),
                (r#""minecraft:a""#, Set::One(a)),
                (r#"["minecraft:a"]"#, Set::List(Box::new([a]))),
                (
                    r#"["minecraft:a","minecraft:b"]"#,
                    Set::List(Box::new([a, b])),
                ),
                ("[]", Set::List(Box::new([]))),
            ];
            for (text, expected) in cases {
                let read: Set = serde_json::from_str(text).unwrap();
                assert_eq!(read, expected, "{text}");
                assert_eq!(serde_json::to_string(&read).unwrap(), text);
            }

            let short: Set = serde_json::from_str(r##""#x""##).unwrap();
            assert_eq!(short, Set::Named(tag(&tags, "minecraft:x")));
            assert_eq!(
                serde_json::to_string(&short).unwrap(),
                r##""#minecraft:x""##
            );

            let listed: Listed = serde_json::from_str(r#"["minecraft:a"]"#).unwrap();
            assert_eq!(listed, Listed::List(Box::new([a])));
            let named: Listed = serde_json::from_str(r##""#x""##).unwrap();
            assert_eq!(named, Listed::Named(tag(&tags, "minecraft:x")));
            assert_eq!(
                serde_json::to_string(&Listed::One(a)).unwrap(),
                r#"["minecraft:a"]"#
            );
            let message = serde_json::from_str::<Listed>(r#""minecraft:a""#)
                .unwrap_err()
                .to_string();
            assert!(message.contains("Not a tag id"), "{message}");
        });
    }

    #[test]
    fn a_one_entry_list_equals_the_bare_entry_and_is_written_as_read() {
        let a = id("minecraft:a");
        let bare = Set::One(a);
        let listed = Set::List(Box::new([a]));
        assert_eq!(bare, listed);
        assert_ne!(bare, Set::List(Box::new([a, a])));
        assert_ne!(bare, Set::List(Box::new([])));
        set().scope(|| {
            assert_eq!(serde_json::to_string(&bare).unwrap(), r#""minecraft:a""#);
            assert_eq!(
                serde_json::to_string(&listed).unwrap(),
                r#"["minecraft:a"]"#
            );
        });
    }

    #[test]
    fn a_holder_set_refuses_an_absent_tag_and_absent_entries() {
        set().scope(|| {
            for text in [
                r##""#minecraft:nowhere""##,
                r#""minecraft:nowhere""#,
                r#"["minecraft:a","minecraft:nowhere"]"#,
            ] {
                let message = serde_json::from_str::<Set>(text).unwrap_err().to_string();
                assert!(message.contains("minecraft:test_marker"), "{message}");
                assert!(message.contains("minecraft:nowhere"), "{message}");
            }
        });
    }

    #[test]
    fn a_holder_set_read_outside_a_scope_fails() {
        for text in [
            r##""#minecraft:t""##,
            r#""minecraft:a""#,
            r#"["minecraft:a"]"#,
        ] {
            let message = serde_json::from_str::<Set>(text).unwrap_err().to_string();
            assert!(
                message.contains("minecraft:test_marker"),
                "{text}: {message}"
            );
        }
    }

    #[test]
    fn a_tag_and_a_list_of_its_members_agree_on_membership() {
        let tags = Tags::<Marker>::new(table(None, &TAGS));
        let registry = markers();
        for (tag_name, members) in [
            ("minecraft:t", vec![id("minecraft:c"), id("minecraft:a")]),
            ("minecraft:none", Vec::new()),
        ] {
            let named = Set::Named(tag(&tags, tag_name));
            let listed = Set::List(members.into_boxed_slice());
            for entry in registry.ids() {
                assert_eq!(
                    named.contains(entry, &tags),
                    listed.contains(entry, &tags),
                    "{tag_name} at {entry:?}"
                );
            }
        }
        let empty = Set::default();
        assert!(registry.ids().all(|entry| !empty.contains(entry, &tags)));
        let nothing = Set::Named(tag(&tags, "minecraft:none"));
        assert!(registry.ids().all(|entry| !nothing.contains(entry, &tags)));
    }

    #[test]
    fn members_come_in_tag_order_and_written_order() {
        let tags = Tags::<Marker>::new(table(None, &TAGS));
        let (a, b, c) = (id("minecraft:a"), id("minecraft:b"), id("minecraft:c"));
        assert_eq!(
            names(Set::Named(tag(&tags, "minecraft:t")).ids(&tags)),
            ["c", "a"]
        );
        assert_eq!(
            names(Set::List(Box::new([c, a, b])).ids(&tags)),
            ["c", "a", "b"]
        );
        assert_eq!(names(Set::One(b).ids(&tags)), ["b"]);
        assert_eq!(names(Set::default().ids(&tags)), [] as [&str; 0]);
    }

    #[test]
    fn a_named_set_answers_from_the_tags_it_is_given() {
        let first = table(None, &TAGS);
        let held = Set::Named(tag(&Tags::<Marker>::new(Arc::clone(&first)), "minecraft:t"));
        let before = Tags::<Marker>::new(Arc::clone(&first));
        assert_eq!(names(held.ids(&before)), ["c", "a"]);

        let rebuilt = table(
            Some(first.names()),
            &[
                ("minecraft:t", r#"{"values":["minecraft:d","minecraft:b"]}"#),
                ("minecraft:y", r#"{"values":["minecraft:a"]}"#),
            ],
        );
        let after = Tags::<Marker>::new(rebuilt);
        assert_eq!(tag(&after, "minecraft:t"), held.tag().unwrap());
        assert_eq!(names(held.ids(&after)), ["d", "b"]);
        for (member, in_before, in_after) in [
            ("minecraft:a", true, false),
            ("minecraft:b", false, true),
            ("minecraft:c", true, false),
            ("minecraft:d", false, true),
        ] {
            assert_eq!(
                held.contains(id(member), &before),
                in_before,
                "{member} before"
            );
            assert_eq!(
                held.contains(id(member), &after),
                in_after,
                "{member} after"
            );
        }

        let removed = Set::Named(tag(&before, "minecraft:x"));
        assert_eq!(names(removed.ids(&after)), [] as [&str; 0]);
        assert!(!removed.contains(id("minecraft:b"), &after));
    }
}
