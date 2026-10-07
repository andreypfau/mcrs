use crate::id::{Id, id_number};
use crate::names::NameTable;
use crate::set::{self, ScopeError};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_key::ResourceKey;
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
    TypeBoundTwice {
        type_name: &'static str,
        first: ResourceLocation<Arc<str>>,
        second: ResourceLocation<Arc<str>>,
    },
    RegistryBoundTwice {
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
            RegistryError::TypeBoundTwice {
                type_name,
                first,
                second,
            } => {
                write!(
                    f,
                    "{type_name} is the type of registry {first} and cannot also be the type of {second}"
                )
            }
            RegistryError::RegistryBoundTwice { registry } => {
                write!(f, "registry {registry} already has a type in this set")
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
impl<R: Send + Sync + 'static> crate::shared::SharedResource for Registry<R> {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.table, &other.table)
    }
}

impl<R> fmt::Debug for Registry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("key", self.table.registry())
            .field("len", &self.len())
            .finish()
    }
}

impl<R> Registry<R> {
    pub fn new(
        key: RegistryKey<R>,
        names: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
    ) -> Result<Self, RegistryError> {
        let table = NameTable::new(key.location().into(), names)?;
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

    pub fn name(&self, id: Id<R>) -> Option<&ResourceLocation<Arc<str>>> {
        self.table.name(id.index())
    }

    pub fn get<S: AsRef<str>>(&self, key: &ResourceKey<R, S>) -> Option<Id<R>> {
        self.table.number(key.as_str()).map(Id::from_number)
    }

    pub fn require<S: AsRef<str>>(&self, key: &ResourceKey<R, S>) -> Result<Id<R>, UnknownEntry> {
        self.get(key).ok_or_else(|| UnknownEntry {
            registry: self.table.registry().clone(),
            name: key.as_str().to_owned(),
        })
    }

    pub fn by_name(&self, name: &str) -> Option<Id<R>> {
        let name = ResourceLocation::read(name).ok()?;
        self.table.number(name.as_str()).map(Id::from_number)
    }

    pub fn require_by_name(&self, name: &str) -> Result<Id<R>, UnknownEntry> {
        self.by_name(name).ok_or_else(|| UnknownEntry {
            registry: self.table.registry().clone(),
            name: name.to_owned(),
        })
    }

    pub fn id(&self, number: u16) -> Option<Id<R>> {
        (usize::from(number) < self.len()).then(|| Id::from_number(number))
    }

    pub fn ids(&self) -> impl Iterator<Item = Id<R>> {
        (0..self.len()).filter_map(id_number).map(Id::from_number)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<R>, &ResourceLocation<Arc<str>>)> {
        self.ids().zip(self.table.names())
    }

    pub fn narrow<N: TryFrom<u16>>(&self, id: Id<R>) -> Result<N, crate::id::NarrowError> {
        id.narrow_in(|| self.table.registry().to_string())
    }
}

impl<R: 'static> Registry<R> {
    pub fn in_scope<T>(
        parsing: &'static str,
        run: impl FnOnce(&Registry<R>) -> T,
    ) -> Result<T, ScopeError> {
        set::in_scope::<R, _, _>(parsing, |set| set.registry::<R>(), run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::rl;

    struct TestRegistry;

    impl TestRegistry {
        const KEY: RegistryKey<TestRegistry> = RegistryKey::new(rl!("minecraft:test_registry"));
    }

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(text).unwrap()
    }

    fn registry(names: &[&str]) -> Registry<TestRegistry> {
        Registry::new(TestRegistry::KEY, names.iter().map(|text| name(text))).unwrap()
    }

    const UNSORTED: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];

    #[test]
    fn ids_are_positions_and_name_their_entries() {
        let registry = registry(&UNSORTED);
        assert_eq!(registry.len(), 3);
        let indices: Vec<usize> = registry.ids().map(Id::index).collect();
        assert_eq!(indices, [0, 1, 2]);
        for (position, text) in UNSORTED.iter().enumerate() {
            let id = registry.by_name(text).unwrap();
            assert_eq!(id.index(), position);
            assert_eq!(registry.name(id).unwrap().as_str(), *text);
        }
    }

    fn build(names: &[&str]) -> Result<Registry<TestRegistry>, RegistryError> {
        Registry::new(TestRegistry::KEY, names.iter().map(|text| name(text)))
    }

    #[test]
    fn index_follows_the_table_order() {
        let registry = registry(&UNSORTED);
        let names: Vec<_> = registry
            .ids()
            .map(|id| registry.name(id).unwrap().as_str())
            .collect();
        assert_eq!(names, UNSORTED);
        assert_eq!(
            registry.by_name("minecraft:desert").map(Id::number),
            Some(1)
        );
        let last = registry.id(2).unwrap();
        assert_eq!(registry.name(last).unwrap().as_str(), "minecraft:forest");
        assert!(registry.id(3).is_none());
    }

    #[test]
    fn an_empty_registry_view_has_no_ids() {
        let registry = registry(&[]);
        assert!(registry.is_empty());
        assert_eq!(registry.ids().count(), 0);
        assert_eq!(registry.by_name("minecraft:plains"), None);
        assert!(registry.id(0).is_none());
    }

    #[test]
    fn a_failed_lookup_names_registry_and_entry() {
        let registry = registry(&UNSORTED);
        let error = registry.require_by_name("minecraft:absent").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("minecraft:test_registry"), "{message}");
        assert!(message.contains("minecraft:absent"), "{message}");
        assert_eq!(error.registry, TestRegistry::KEY.location());
        assert_eq!(error.name, "minecraft:absent");
        assert_eq!(
            registry.require_by_name("minecraft:desert").unwrap(),
            registry.by_name("minecraft:desert").unwrap()
        );
    }

    fn key(text: &'static str) -> ResourceKey<TestRegistry, &'static str> {
        ResourceKey::new(ResourceLocation::new_static(text))
    }

    #[test]
    fn typed_lookups_follow_vanilla() {
        let registry = registry(&UNSORTED);
        for (text, number) in [
            ("minecraft:plains", Some(0)),
            ("minecraft:desert", Some(1)),
            ("minecraft:forest", Some(2)),
            ("minecraft:absent", None),
        ] {
            let key = key(text);
            assert_eq!(registry.get(&key).map(Id::number), number, "{text}");
            match number {
                Some(_) => assert_eq!(registry.require(&key), Ok(registry.get(&key).unwrap())),
                None => {
                    let error = registry.require(&key).unwrap_err();
                    let message = error.to_string();
                    assert!(message.contains("minecraft:test_registry"), "{message}");
                    assert!(message.contains(text), "{message}");
                }
            }
        }

        for (text, number) in [
            ("plains", Some(0)),
            ("minecraft:plains", Some(0)),
            ("Plains", None),
            ("minecraft:Plains", None),
            ("absent", None),
        ] {
            assert_eq!(registry.by_name(text).map(Id::number), number, "{text}");
            assert_eq!(
                registry.require_by_name(text).ok().map(Id::number),
                number,
                "{text}"
            );
        }
        let error = registry.require_by_name("Plains").unwrap_err();
        assert_eq!(error.name, "Plains");
        assert_eq!(error.registry, TestRegistry::KEY.location());

        for id in registry.ids() {
            let name = registry.name(id).unwrap().clone();
            assert_eq!(registry.get(&ResourceKey::from_location(name)), Some(id));
        }
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
        let first = larger.by_name("minecraft:plains").unwrap();
        let second = larger.by_name("minecraft:desert").unwrap();
        let third = larger.by_name("minecraft:forest").unwrap();
        assert!(smaller.name(first).is_some());
        assert!(smaller.name(second).is_none());
        assert!(smaller.name(third).is_none());
    }
}
