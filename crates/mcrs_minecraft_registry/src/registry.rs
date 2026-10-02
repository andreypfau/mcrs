use crate::id::{Id, id_number};
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
        }
    }
}

impl std::error::Error for RegistryError {}

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
        Ok(Registry {
            table: Arc::new(Table {
                names,
                numbers,
                tags: tags.into_iter().collect(),
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

    pub fn ids(&self) -> impl Iterator<Item = Id<R>> {
        (0..self.len()).filter_map(id_number).map(Id::from_number)
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
}
