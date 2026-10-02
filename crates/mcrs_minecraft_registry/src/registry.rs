use crate::id::{Id, id_number};
use crate::set::{self, ScopeError};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateEntry {
        registry: ResourceLocation<&'static str>,
        name: ResourceLocation<Arc<str>>,
    },
    DuplicateTag {
        registry: ResourceLocation<&'static str>,
        tag: ResourceLocation<Arc<str>>,
    },
    TooManyEntries {
        registry: ResourceLocation<&'static str>,
        len: usize,
    },
    LengthMismatch {
        registry: ResourceLocation<&'static str>,
        expected: usize,
        found: usize,
    },
    DuplicateRegistry {
        registry: ResourceLocation<&'static str>,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateEntry { registry, name } => {
                write!(f, "registry {registry} lists the entry {name} twice")
            }
            RegistryError::DuplicateTag { registry, tag } => {
                write!(f, "registry {registry} lists the tag {tag} twice")
            }
            RegistryError::TooManyEntries { registry, len } => {
                write!(
                    f,
                    "registry {registry} has {len} entries, more than an id can number"
                )
            }
            RegistryError::LengthMismatch {
                registry,
                expected,
                found,
            } => {
                write!(
                    f,
                    "registry {registry} has {expected} entries but {found} values were given"
                )
            }
            RegistryError::DuplicateRegistry { registry } => {
                write!(f, "a registry set already holds the registry {registry}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEntry {
    pub registry: ResourceLocation<&'static str>,
    pub name: String,
}

impl fmt::Display for UnknownEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "registry {} holds no entry named {}",
            self.registry, self.name
        )
    }
}

impl std::error::Error for UnknownEntry {}

struct Table {
    names: Vec<ResourceLocation<Arc<str>>>,
    numbers: HashMap<ResourceLocation<Arc<str>>, u32>,
    tags: HashSet<ResourceLocation<Arc<str>>>,
}

pub struct Registry<R> {
    table: Arc<Table>,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for Registry<R> {
    fn clone(&self) -> Self {
        Registry {
            table: Arc::clone(&self.table),
            _marker: PhantomData,
        }
    }
}

impl<R: RegistryKey> Registry<R> {
    pub fn new(
        names: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
        tags: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
    ) -> Result<Self, RegistryError> {
        let names: Vec<_> = names.into_iter().collect();
        let mut numbers = HashMap::with_capacity(names.len());
        for (position, name) in names.iter().enumerate() {
            let number = id_number(position).ok_or(RegistryError::TooManyEntries {
                registry: R::KEY,
                len: names.len(),
            })?;
            if numbers.insert(name.clone(), number).is_some() {
                return Err(RegistryError::DuplicateEntry {
                    registry: R::KEY,
                    name: name.clone(),
                });
            }
        }
        let mut tag_names = HashSet::new();
        for tag in tags {
            if !tag_names.insert(tag.clone()) {
                return Err(RegistryError::DuplicateTag {
                    registry: R::KEY,
                    tag,
                });
            }
        }
        Ok(Registry {
            table: Arc::new(Table {
                names,
                numbers,
                tags: tag_names,
            }),
            _marker: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.table.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.names.is_empty()
    }

    pub fn key(&self, id: Id<R>) -> Option<&ResourceLocation<Arc<str>>> {
        self.table.names.get(id.index())
    }

    pub fn get(&self, name: &str) -> Option<Id<R>> {
        self.table.numbers.get(name).copied().map(Id::from_number)
    }

    pub fn require(&self, name: &str) -> Result<Id<R>, UnknownEntry> {
        self.get(name).ok_or_else(|| UnknownEntry {
            registry: R::KEY,
            name: name.to_owned(),
        })
    }

    pub fn has_tag(&self, name: &str) -> bool {
        self.table.tags.contains(name)
    }

    pub fn ids(&self) -> impl Iterator<Item = Id<R>> {
        (0..self.len()).filter_map(id_number).map(Id::from_number)
    }

    pub fn in_scope<T>(
        parsing: &'static str,
        run: impl FnOnce(&Registry<R>) -> T,
    ) -> Result<T, ScopeError> {
        let set = set::current().ok_or(ScopeError::NoScope {
            parsing,
            registry: R::KEY,
        })?;
        let registry = set.registry::<R>().ok_or(ScopeError::MissingRegistry {
            parsing,
            registry: R::KEY,
        })?;
        Ok(run(&registry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::rl;

    struct TestRegistry;

    impl RegistryKey for TestRegistry {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_registry");
    }

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(text).unwrap()
    }

    fn registry(names: &[&str]) -> Registry<TestRegistry> {
        Registry::new(names.iter().map(|text| name(text)), std::iter::empty()).unwrap()
    }

    const UNSORTED: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];

    #[test]
    fn an_id_is_the_position_of_its_name() {
        let registry = registry(&UNSORTED);
        for (position, text) in UNSORTED.iter().enumerate() {
            assert_eq!(registry.get(text).unwrap().index(), position);
        }
    }

    #[test]
    fn ids_iterate_in_id_order() {
        let registry = registry(&UNSORTED);
        let indices: Vec<usize> = registry.ids().map(Id::index).collect();
        assert_eq!(indices, [0, 1, 2]);
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn the_name_of_an_id_is_read_back() {
        let registry = registry(&UNSORTED);
        for text in UNSORTED {
            let id = registry.get(text).unwrap();
            assert_eq!(registry.key(id).unwrap().as_str(), text);
        }
    }

    #[test]
    fn an_unknown_name_has_no_id() {
        let registry = registry(&UNSORTED);
        assert!(registry.get("minecraft:absent").is_none());
    }

    fn build(names: &[&str], tags: &[&str]) -> Result<Registry<TestRegistry>, RegistryError> {
        Registry::new(
            names.iter().map(|text| name(text)),
            tags.iter().map(|text| name(text)),
        )
    }

    #[test]
    fn a_failed_lookup_names_registry_and_entry() {
        let registry = registry(&UNSORTED);
        let error = registry.require("minecraft:absent").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("minecraft:test_registry"), "{message}");
        assert!(message.contains("minecraft:absent"), "{message}");
        assert_eq!(error.registry, TestRegistry::KEY);
        assert_eq!(error.name, "minecraft:absent");
        assert_eq!(
            registry.require("minecraft:desert").unwrap(),
            registry.get("minecraft:desert").unwrap()
        );
    }

    #[test]
    fn a_duplicate_name_does_not_build() {
        let error = build(&["minecraft:a", "minecraft:b", "minecraft:a"], &[])
            .err()
            .unwrap();
        let message = error.to_string();
        assert!(
            matches!(error, RegistryError::DuplicateEntry { .. }),
            "{message}"
        );
        assert!(message.contains("minecraft:a"), "{message}");
        assert!(message.contains("minecraft:test_registry"), "{message}");
    }

    #[test]
    fn a_duplicate_tag_does_not_build() {
        let error = build(&["minecraft:a"], &["minecraft:t", "minecraft:t"])
            .err()
            .unwrap();
        let message = error.to_string();
        assert!(
            matches!(error, RegistryError::DuplicateTag { .. }),
            "{message}"
        );
        assert!(message.contains("minecraft:t"), "{message}");
    }

    #[test]
    fn an_empty_registry_builds_and_holds_nothing() {
        let registry = registry(&[]);
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
        assert_eq!(registry.ids().count(), 0);
        assert!(registry.get("minecraft:anything").is_none());
        assert!(registry.require("minecraft:anything").is_err());
    }

    #[test]
    fn a_registry_knows_its_tag_names() {
        let registry = build(&["minecraft:a"], &["minecraft:t", "minecraft:u"]).unwrap();
        assert!(registry.has_tag("minecraft:t"));
        assert!(registry.has_tag("minecraft:u"));
        assert!(!registry.has_tag("minecraft:v"));
    }

    #[test]
    fn a_tag_name_is_not_an_entry_name() {
        let registry = build(&["minecraft:a"], &["minecraft:t"]).unwrap();
        assert!(!registry.has_tag("minecraft:a"));
        assert!(registry.get("minecraft:t").is_none());
    }

    #[test]
    fn names_compare_as_the_exact_text() {
        let registry = registry(&["minecraft:alpha"]);
        assert!(registry.get("minecraft:alpha").is_some());
        assert!(registry.get("alpha").is_none());
        assert!(registry.get("Minecraft:alpha").is_none());
        assert!(registry.get("minecraft:Alpha").is_none());
        assert!(registry.require("alpha").is_err());
    }

    #[test]
    fn an_id_of_a_larger_registry_has_no_name_in_a_smaller_one() {
        let larger = registry(&UNSORTED);
        let smaller = registry(&["minecraft:plains"]);
        let first = larger.get("minecraft:plains").unwrap();
        let second = larger.get("minecraft:desert").unwrap();
        let third = larger.get("minecraft:forest").unwrap();
        assert!(smaller.key(first).is_some());
        assert!(smaller.key(second).is_none());
        assert!(smaller.key(third).is_none());
    }

    #[test]
    fn the_first_and_the_last_id_bound_the_table() {
        let registry = registry(&UNSORTED);
        let ids: Vec<Id<TestRegistry>> = registry.ids().collect();
        assert_eq!(ids.len(), registry.len());
        assert_eq!(ids.first().unwrap().index(), 0);
        assert_eq!(ids.last().unwrap().index(), registry.len() - 1);
        assert_eq!(registry.get("minecraft:plains"), ids.first().copied());
        assert_eq!(registry.get("minecraft:forest"), ids.last().copied());
    }
}
