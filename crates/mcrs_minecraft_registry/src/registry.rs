use crate::id::{Id, id_number};
use crate::names::NameTable;
use crate::set::{self, ScopeError};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateEntry {
        registry: ResourceLocation<Arc<str>>,
        name: ResourceLocation<Arc<str>>,
    },
    TooManyEntries {
        registry: ResourceLocation<Arc<str>>,
        len: usize,
    },
    LengthMismatch {
        registry: ResourceLocation<Arc<str>>,
        expected: usize,
        found: usize,
    },
    DuplicateRegistry {
        registry: ResourceLocation<Arc<str>>,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateEntry { registry, name } => {
                write!(f, "registry {registry} lists the entry {name} twice")
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
    pub registry: ResourceLocation<Arc<str>>,
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

#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct Registry<R> {
    table: Arc<NameTable>,
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

#[cfg(feature = "bevy")]
impl<R: RegistryKey + Send + Sync + 'static> crate::shared::SharedResource for Registry<R> {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.table, &other.table)
    }
}

impl<R: RegistryKey> fmt::Debug for Registry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("key", &R::KEY)
            .field("len", &self.len())
            .finish()
    }
}

impl<R: RegistryKey> Registry<R> {
    pub fn new(
        names: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
    ) -> Result<Self, RegistryError> {
        let table = NameTable::new(R::KEY.into(), names)?;
        Ok(Self::view(Arc::new(table)))
    }

    pub(crate) fn view(table: Arc<NameTable>) -> Self {
        Registry {
            table,
            _marker: PhantomData,
        }
    }

    pub fn table(&self) -> &Arc<NameTable> {
        &self.table
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn key(&self, id: Id<R>) -> Option<&ResourceLocation<Arc<str>>> {
        self.table.name(id.index())
    }

    pub fn get(&self, name: &str) -> Option<Id<R>> {
        self.table.number(name).map(Id::from_number)
    }

    pub fn id(&self, number: u16) -> Option<Id<R>> {
        (usize::from(number) < self.len()).then(|| Id::from_number(number))
    }

    pub fn require(&self, name: &str) -> Result<Id<R>, UnknownEntry> {
        self.get(name).ok_or_else(|| UnknownEntry {
            registry: R::KEY.into(),
            name: name.to_owned(),
        })
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
        Registry::new(names.iter().map(|text| name(text))).unwrap()
    }

    const UNSORTED: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];

    #[test]
    fn ids_are_positions_and_name_their_entries() {
        let registry = registry(&UNSORTED);
        assert_eq!(registry.len(), 3);
        let indices: Vec<usize> = registry.ids().map(Id::index).collect();
        assert_eq!(indices, [0, 1, 2]);
        for (position, text) in UNSORTED.iter().enumerate() {
            let id = registry.get(text).unwrap();
            assert_eq!(id.index(), position);
            assert_eq!(registry.key(id).unwrap().as_str(), *text);
        }
    }

    fn build(names: &[&str]) -> Result<Registry<TestRegistry>, RegistryError> {
        Registry::new(names.iter().map(|text| name(text)))
    }

    #[test]
    fn index_follows_the_table_order() {
        let registry = registry(&UNSORTED);
        let names: Vec<_> = registry
            .ids()
            .map(|id| registry.key(id).unwrap().as_str())
            .collect();
        assert_eq!(names, UNSORTED);
        assert_eq!(registry.get("minecraft:desert").map(Id::number), Some(1));
        let last = registry.id(2).unwrap();
        assert_eq!(registry.key(last).unwrap().as_str(), "minecraft:forest");
        assert!(registry.id(3).is_none());
    }

    #[test]
    fn an_empty_registry_view_has_no_ids() {
        let registry = registry(&[]);
        assert!(registry.is_empty());
        assert_eq!(registry.ids().count(), 0);
        assert_eq!(registry.get("minecraft:plains"), None);
        assert!(registry.id(0).is_none());
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
        let error = build(&["minecraft:a", "minecraft:b", "minecraft:a"])
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
}
