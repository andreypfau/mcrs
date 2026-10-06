use crate::entries::Entries;
use crate::names::NameTable;
use crate::registry::{Registry, RegistryError};
use crate::tags::{TagTable, Tags};
use mcrs_minecraft_core::registry_key::{RegistryKey, TypeBinding};
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::any::{Any, TypeId, type_name};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

type Tables = HashMap<ResourceLocation<Arc<str>>, Arc<NameTable>>;
type Paths = HashMap<Box<str>, Arc<NameTable>>;
type Types = HashMap<TypeId, ResourceLocation<Arc<str>>>;
pub(crate) type Column = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) struct Values {
    pub(crate) tags: HashMap<ResourceLocation<Arc<str>>, Arc<TagTable>>,
    pub(crate) columns: HashMap<ResourceLocation<Arc<str>>, Column>,
    pub(crate) origins: HashMap<ResourceLocation<Arc<str>>, Box<[u32]>>,
    pub(crate) packs: Box<[Box<str>]>,
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct RegistrySet {
    tables: Arc<Tables>,
    paths: Arc<Paths>,
    types: Arc<Types>,
    values: Arc<Values>,
}

#[cfg(feature = "bevy")]
impl crate::shared::SharedResource for RegistrySet {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tables, &other.tables)
    }
}

thread_local! {
    static CURRENT: RefCell<Option<RegistrySet>> = const { RefCell::new(None) };
}

impl RegistrySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_tables(
        tables: impl IntoIterator<Item = Arc<NameTable>>,
    ) -> Result<Self, RegistryError> {
        let mut by_name = Tables::new();
        for table in tables {
            let registry = table.registry().clone();
            if by_name.insert(registry.clone(), table).is_some() {
                return Err(RegistryError::DuplicateRegistry { registry });
            }
        }
        Ok(Self::of(by_name, Arc::default()))
    }

    pub fn from_locations(
        registries: &[(
            ResourceLocation<&'static str>,
            &[ResourceLocation<&'static str>],
        )],
    ) -> Result<Self, RegistryError> {
        let mut tables = Vec::with_capacity(registries.len());
        for &(registry, names) in registries {
            let names = names.iter().map(|&name| name.into());
            tables.push(Arc::new(NameTable::new(registry.into(), names)?));
        }
        Self::from_tables(tables)
    }

    fn of(tables: Tables, values: Arc<Values>) -> Self {
        let paths = tables
            .iter()
            .map(|(registry, table)| (registry.path().into(), Arc::clone(table)))
            .collect();
        RegistrySet {
            tables: Arc::new(tables),
            paths: Arc::new(paths),
            types: Arc::default(),
            values,
        }
    }

    pub(crate) fn with_values(self, values: Values) -> Self {
        RegistrySet {
            values: Arc::new(values),
            ..self
        }
    }

    pub fn with<R: 'static>(self, registry: Registry<R>) -> Result<Self, RegistryError> {
        let name = registry.table().registry().clone();
        if self.tables.contains_key(&name) {
            return Err(RegistryError::DuplicateRegistry { registry: name });
        }
        let mut tables = (*self.tables).clone();
        tables.insert(name.clone(), Arc::clone(registry.table()));
        let types = Arc::clone(&self.types);
        RegistrySet {
            types,
            ..Self::of(tables, Arc::clone(&self.values))
        }
        .bind(TypeId::of::<R>(), type_name::<R>(), name)
    }

    pub fn with_types(
        self,
        bindings: impl IntoIterator<Item = TypeBinding>,
    ) -> Result<Self, RegistryError> {
        bindings.into_iter().try_fold(self, |set, binding| {
            set.bind(binding.type_id, binding.type_name, binding.registry.into())
        })
    }

    pub(crate) fn with_types_of(self, other: &RegistrySet) -> Self {
        RegistrySet {
            types: Arc::clone(&other.types),
            ..self
        }
    }

    fn bind(
        mut self,
        type_id: TypeId,
        type_name: &'static str,
        registry: ResourceLocation<Arc<str>>,
    ) -> Result<Self, RegistryError> {
        if let Some(bound) = self.types.get(&type_id) {
            return if *bound == registry {
                Ok(self)
            } else {
                Err(RegistryError::TypeBoundTwice {
                    type_name,
                    first: bound.clone(),
                    second: registry,
                })
            };
        }
        if let Some((_, other)) = self.types.iter().find(|(_, bound)| **bound == registry) {
            return Err(RegistryError::RegistryBoundTwice {
                registry: other.clone(),
            });
        }
        Arc::make_mut(&mut self.types).insert(type_id, registry);
        Ok(self)
    }

    fn name_of<R: 'static>(&self) -> Option<&ResourceLocation<Arc<str>>> {
        self.types.get(&TypeId::of::<R>())
    }

    pub fn registry<R: 'static>(&self) -> Option<Registry<R>> {
        self.tables
            .get(self.name_of::<R>()?)
            .map(|table| Registry::view(Arc::clone(table)))
    }

    pub fn registry_of<R>(&self, key: RegistryKey<R>) -> Option<Registry<R>> {
        self.tables
            .get(key.location().as_static_str())
            .map(|table| Registry::view(Arc::clone(table)))
    }

    pub fn table(&self, registry: &str) -> Option<&Arc<NameTable>> {
        self.tables.get(registry)
    }

    pub fn tags<R: 'static>(&self) -> Option<Tags<R>> {
        self.values
            .tags
            .get(self.name_of::<R>()?)
            .map(|table| Tags::new(Arc::clone(table)))
    }

    pub fn tag_table(&self, registry: &str) -> Option<&Arc<TagTable>> {
        self.values.tags.get(registry)
    }

    pub fn with_tags(self, table: Arc<TagTable>) -> Self {
        let mut values = (*self.values).clone();
        values.tags.insert(table.registry().clone(), table);
        self.with_values(values)
    }

    pub(crate) fn table_at_path(&self, path: &str) -> Option<&NameTable> {
        self.paths.get(path).map(|table| &**table)
    }

    pub fn tables(&self) -> impl Iterator<Item = &Arc<NameTable>> {
        self.tables.values()
    }

    pub fn column<T: 'static>(&self, registry: &str) -> Option<&[T]> {
        self.column_any(registry)?
            .downcast_ref::<Arc<[T]>>()
            .map(|values| &**values)
    }

    pub fn entries<R: 'static, T: 'static>(&self) -> Option<Entries<R, T>> {
        self.column_any(self.name_of::<R>()?.as_str())?
            .downcast_ref::<Arc<[T]>>()
            .map(|values| Entries::from_shared(Arc::clone(values)))
    }

    pub fn pack_of(&self, registry: &str, id: usize) -> Option<&str> {
        let pack = *self.values.origins.get(registry)?.get(id)?;
        self.values.packs.get(pack as usize).map(|name| &**name)
    }

    pub(crate) fn column_any(&self, registry: &str) -> Option<&Column> {
        self.values.columns.get(registry)
    }

    pub fn scope<T>(&self, run: impl FnOnce() -> T) -> T {
        struct Restore(Option<RegistrySet>);

        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                CURRENT.with_borrow_mut(|current| *current = previous);
            }
        }

        let already = CURRENT.with_borrow(|current| {
            current.as_ref().is_some_and(|current| {
                Arc::ptr_eq(&current.tables, &self.tables)
                    && Arc::ptr_eq(&current.types, &self.types)
                    && Arc::ptr_eq(&current.values, &self.values)
            })
        });
        if already {
            return run();
        }
        let previous = CURRENT.with_borrow_mut(|current| current.replace(self.clone()));
        let _restore = Restore(previous);
        run()
    }
}

pub(crate) fn current() -> Option<RegistrySet> {
    CURRENT.with_borrow(Clone::clone)
}

pub(crate) fn label<R: 'static>() -> String {
    CURRENT.with_borrow(|current| {
        current
            .as_ref()
            .and_then(|set| set.name_of::<R>())
            .map_or_else(|| type_name::<R>().to_owned(), ToString::to_string)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    NoScope {
        parsing: &'static str,
        registry: String,
    },
    MissingRegistry {
        parsing: &'static str,
        registry: String,
    },
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::NoScope { parsing, registry } => write!(
                f,
                "cannot resolve {parsing} against registry {registry}: no registry scope is active on this thread"
            ),
            ScopeError::MissingRegistry { parsing, registry } => write!(
                f,
                "cannot resolve {parsing} against registry {registry}: the active registry scope holds no such registry"
            ),
        }
    }
}

impl std::error::Error for ScopeError {}

const _: fn() = || {
    fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<RegistrySet>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holder::{Holder, HolderWireOnly};
    use crate::holder_set::HolderSet;
    use crate::id::Id;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::{RegistryValue, rl};
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Deserializer, Serialize};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Barrier;

    struct Biome;

    impl Biome {
        const KEY: RegistryKey<Biome> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
    }

    struct Item;

    impl Item {
        const KEY: RegistryKey<Item> = RegistryKey::new(rl!("minecraft:item"));
    }

    const PLAINS_FIRST: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];
    const PLAINS_LAST: [&str; 3] = ["minecraft:desert", "minecraft:forest", "minecraft:plains"];

    fn set_of(names: &[&str]) -> RegistrySet {
        RegistrySet::new()
            .with(registry(Biome::KEY, names))
            .unwrap()
    }

    fn parse(name: &str) -> Result<Id<Biome>, serde_json::Error> {
        serde_json::from_str(&format!("\"{name}\""))
    }

    fn index_of(name: &str) -> usize {
        parse(name).unwrap().index()
    }

    fn registry<R>(key: RegistryKey<R>, names: &[&str]) -> Registry<R> {
        Registry::new(
            key,
            names
                .iter()
                .map(|text| ResourceLocation::read(text).unwrap()),
        )
        .unwrap()
    }

    fn no_scope_here() -> bool {
        Registry::<Biome>::in_scope("probe", |_| ())
            == Err(ScopeError::NoScope {
                parsing: "probe",
                registry: type_name::<Biome>().to_owned(),
            })
    }

    #[test]
    fn a_name_parses_to_its_id_inside_a_scope_and_is_written_back_as_the_name() {
        assert!(no_scope_here());
        let biomes = registry(Biome::KEY, &["minecraft:plains", "minecraft:desert"]);
        let set = RegistrySet::new().with(biomes.clone()).unwrap();
        set.scope(|| {
            let id: Id<Biome> = serde_json::from_str("\"minecraft:desert\"").unwrap();
            assert_eq!(Some(id), biomes.by_name("minecraft:desert"));
            assert_eq!(id.index(), 1);
            assert_eq!(serde_json::to_string(&id).unwrap(), "\"minecraft:desert\"");
        });
        assert!(no_scope_here());
    }

    #[test]
    fn parsing_outside_a_scope_fails_naming_the_type_and_the_registry() {
        assert!(no_scope_here());
        let error = serde_json::from_str::<Id<Biome>>("\"minecraft:plains\"").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Id<"), "{message}");
        assert!(message.contains("tests::Biome"), "{message}");
        let probe = Registry::<Biome>::in_scope("probe", |_| ());
        assert!(matches!(probe, Err(ScopeError::NoScope { .. })));
        assert!(no_scope_here());
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Entry {
        volume: i32,
    }

    impl RegistryValue for Entry {
        type Registry = Biome;
    }

    fn refuses_without_the_registry<T: DeserializeOwned + Serialize>(
        label: &str,
        of: fn(Id<Biome>) -> T,
    ) {
        let id = registry(Biome::KEY, &PLAINS_FIRST)
            .by_name("minecraft:plains")
            .unwrap();
        let value = of(id);
        let without_biomes = RegistrySet::new()
            .with(registry(Item::KEY, &["minecraft:stick"]))
            .unwrap();

        for (set, expected) in [
            (None, "no registry scope is active"),
            (Some(without_biomes), "holds no such registry"),
        ] {
            let attempt = || {
                (
                    serde_json::from_str::<T>("\"minecraft:plains\"").err(),
                    serde_json::to_string(&value).err(),
                )
            };
            let (read, written) = match &set {
                Some(set) => set.scope(attempt),
                None => attempt(),
            };
            for (direction, error) in [("read", read), ("write", written)] {
                let message = error
                    .unwrap_or_else(|| panic!("{label} {direction} succeeded: {expected}"))
                    .to_string();
                assert!(message.contains(expected), "{label} {direction}: {message}");
                assert!(
                    message.contains("tests::Biome"),
                    "{label} {direction}: {message}"
                );
            }
        }
    }

    #[test]
    fn scoped_types_refuse_to_work_without_a_scope() {
        refuses_without_the_registry("Id", |id| id);
        refuses_without_the_registry("Holder", Holder::<Entry>::Reference);
        refuses_without_the_registry("HolderWireOnly", |id| {
            HolderWireOnly(Holder::<Entry>::Reference(id))
        });
        refuses_without_the_registry("HolderSet", HolderSet::<Biome>::One);
    }

    #[test]
    fn a_thread_spawned_inside_a_scope_has_none() {
        assert!(no_scope_here());
        let outer = set_of(&PLAINS_FIRST);
        outer.scope(|| {
            std::thread::scope(|threads| {
                threads
                    .spawn(|| {
                        assert!(no_scope_here());
                        let message = parse("minecraft:plains").unwrap_err().to_string();
                        assert!(message.contains("Id<"), "{message}");
                        assert!(message.contains("tests::Biome"), "{message}");
                        assert!(no_scope_here());
                    })
                    .join()
                    .unwrap();
            });
            assert_eq!(index_of("minecraft:plains"), 0);
            assert_eq!(index_of("minecraft:forest"), 2);
        });
        assert!(no_scope_here());
    }

    #[test]
    fn a_nested_scope_restores_the_outer_set() {
        assert!(no_scope_here());
        let outer = set_of(&PLAINS_FIRST);
        let inner = set_of(&PLAINS_LAST);
        outer.scope(|| {
            assert_eq!(index_of("minecraft:plains"), 0);
            inner.scope(|| assert_eq!(index_of("minecraft:plains"), 2));
            assert_eq!(index_of("minecraft:plains"), 0);
        });
        assert!(no_scope_here());
    }

    #[test]
    fn a_panic_inside_a_scope_restores_what_was_there() {
        assert!(no_scope_here());
        let outer = set_of(&PLAINS_FIRST);
        let inner = set_of(&PLAINS_LAST);
        outer.scope(|| {
            let unwound = catch_unwind(AssertUnwindSafe(|| {
                inner.scope(|| {
                    assert_eq!(index_of("minecraft:plains"), 2);
                    panic!("unwinding out of a scope");
                })
            }));
            assert!(unwound.is_err());
            assert_eq!(index_of("minecraft:plains"), 0);
        });
        assert!(no_scope_here());
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            outer.scope(|| {
                assert_eq!(index_of("minecraft:plains"), 0);
                panic!("unwinding out of the outermost scope");
            })
        }));
        assert!(unwound.is_err());
        assert!(no_scope_here());
    }

    #[test]
    fn two_threads_with_two_sets_resolve_to_their_own_ids() {
        assert!(no_scope_here());
        let first = set_of(&PLAINS_FIRST);
        let second = set_of(&PLAINS_LAST);
        let both_entered = Barrier::new(2);
        std::thread::scope(|threads| {
            for (set, expected) in [(&first, 0), (&second, 2)] {
                let both_entered = &both_entered;
                threads.spawn(move || {
                    assert!(no_scope_here());
                    set.scope(|| {
                        both_entered.wait();
                        for _ in 0..100 {
                            assert_eq!(index_of("minecraft:plains"), expected);
                        }
                    });
                    assert!(no_scope_here());
                });
            }
        });
        assert!(no_scope_here());
    }

    #[test]
    fn scopes_unwind_in_reverse_order() {
        assert!(no_scope_here());
        let a = set_of(&PLAINS_FIRST);
        let b = set_of(&PLAINS_LAST);
        let mut seen = Vec::new();
        a.scope(|| {
            seen.push(index_of("minecraft:plains"));
            b.scope(|| {
                seen.push(index_of("minecraft:plains"));
                a.scope(|| seen.push(index_of("minecraft:plains")));
                seen.push(index_of("minecraft:plains"));
            });
            seen.push(index_of("minecraft:plains"));
        });
        assert_eq!(seen, [0, 2, 0, 2, 0]);
        assert!(no_scope_here());
    }

    #[test]
    fn entering_the_current_set_again_is_harmless() {
        assert!(no_scope_here());
        let set = set_of(&PLAINS_FIRST);
        set.scope(|| {
            set.scope(|| assert_eq!(index_of("minecraft:desert"), 1));
            assert_eq!(index_of("minecraft:desert"), 1);
        });
        assert!(no_scope_here());
    }

    #[test]
    fn a_scope_over_a_set_without_the_registry_is_a_distinct_error() {
        assert!(no_scope_here());
        let items_only = RegistrySet::new()
            .with(registry(Item::KEY, &["minecraft:stick"]))
            .unwrap();
        for set in [RegistrySet::new(), items_only] {
            set.scope(|| {
                let message = parse("minecraft:plains").unwrap_err().to_string();
                assert!(message.contains("Id<"), "{message}");
                assert!(message.contains("tests::Biome"), "{message}");
                let missing = Registry::<Biome>::in_scope("probe", |_| ()).unwrap_err();
                assert_eq!(
                    missing,
                    ScopeError::MissingRegistry {
                        parsing: "probe",
                        registry: type_name::<Biome>().to_owned(),
                    }
                );
                assert_ne!(
                    missing,
                    ScopeError::NoScope {
                        parsing: "probe",
                        registry: type_name::<Biome>().to_owned(),
                    }
                );
                assert!(!message.contains("no registry scope"), "{message}");
            });
            assert!(no_scope_here());
        }
    }

    #[test]
    fn two_registries_of_one_key_cannot_share_a_set() {
        assert!(no_scope_here());
        let set = RegistrySet::new()
            .with(registry(Biome::KEY, &PLAINS_FIRST))
            .unwrap();
        let error = set
            .clone()
            .with(registry(Biome::KEY, &PLAINS_LAST))
            .err()
            .unwrap();
        assert_eq!(
            error,
            RegistryError::DuplicateRegistry {
                registry: Biome::KEY.location().into()
            }
        );
        assert!(error.to_string().contains("minecraft:worldgen/biome"));
        assert!(set.registry::<Biome>().is_some());
        assert!(set.registry::<Item>().is_none());
        assert!(no_scope_here());
    }

    #[test]
    fn a_name_without_its_namespace_parses_and_is_written_in_full() {
        assert!(no_scope_here());
        let set = set_of(&PLAINS_FIRST);
        set.scope(|| {
            let bare = parse("desert").unwrap();
            assert_eq!(bare, parse("minecraft:desert").unwrap());
            assert_eq!(bare.index(), 1);
            assert_eq!(
                serde_json::to_string(&bare).unwrap(),
                "\"minecraft:desert\""
            );
        });
        assert!(no_scope_here());
    }

    #[test]
    fn an_unknown_name_inside_a_scope_names_registry_and_entry() {
        assert!(no_scope_here());
        let set = set_of(&PLAINS_FIRST);
        set.scope(|| {
            for text in ["minecraft:nowhere", "nowhere"] {
                let message = parse(text).unwrap_err().to_string();
                assert!(message.contains("minecraft:worldgen/biome"), "{message}");
                assert!(message.contains("minecraft:nowhere"), "{message}");
            }
        });
        assert!(no_scope_here());
    }

    #[test]
    fn an_id_of_another_set_does_not_serialize_in_a_smaller_one() {
        assert!(no_scope_here());
        let larger = set_of(&PLAINS_FIRST);
        let smaller = set_of(&["minecraft:plains"]);
        let (first, last) = larger.scope(|| {
            (
                parse("minecraft:plains").unwrap(),
                parse("minecraft:forest").unwrap(),
            )
        });
        assert_eq!(last.index(), 2);
        smaller.scope(|| {
            assert_eq!(
                serde_json::to_string(&first).unwrap(),
                "\"minecraft:plains\""
            );
            let message = serde_json::to_string(&last).unwrap_err().to_string();
            assert!(message.contains("minecraft:worldgen/biome"), "{message}");
            assert!(message.contains('2'), "{message}");
        });
        assert!(no_scope_here());
    }

    #[test]
    fn a_scope_entered_from_inside_a_parse_does_not_panic() {
        struct Reentrant(usize);

        impl<'de> Deserialize<'de> for Reentrant {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let id = Id::<Biome>::deserialize(deserializer)?;
                let inner = set_of(&PLAINS_LAST);
                Registry::<Biome>::in_scope("reentrant", |_| {
                    inner.scope(|| Reentrant(index_of("minecraft:plains") * 10 + id.index()))
                })
                .map_err(serde::de::Error::custom)
            }
        }

        assert!(no_scope_here());
        let outer = set_of(&PLAINS_FIRST);
        outer.scope(|| {
            let parsed: Reentrant = serde_json::from_str("\"minecraft:desert\"").unwrap();
            assert_eq!(parsed.0, 21);
            assert_eq!(index_of("minecraft:plains"), 0);
        });
        assert!(no_scope_here());
    }

    #[test]
    fn locations_build_tables_numbered_in_list_order_and_bad_lists_are_refused() {
        let plains_last = [
            rl!("minecraft:desert"),
            rl!("minecraft:forest"),
            rl!("minecraft:plains"),
        ];
        let set = RegistrySet::from_locations(&[(rl!("minecraft:item"), &plains_last)]).unwrap();
        let table = set.table("minecraft:item").unwrap();
        assert_eq!(table.number("minecraft:plains"), Some(2));
        assert_eq!(table.name(0).unwrap().as_str(), "minecraft:desert");

        let twice = [rl!("minecraft:a"), rl!("minecraft:a")];
        let cases: [(
            &str,
            &[(
                ResourceLocation<&'static str>,
                &[ResourceLocation<&'static str>],
            )],
            &str,
        ); 2] = [
            (
                "an entry twice",
                &[(rl!("minecraft:item"), &twice)],
                "minecraft:a",
            ),
            (
                "a registry twice",
                &[(rl!("minecraft:item"), &[]), (rl!("minecraft:item"), &[])],
                "minecraft:item",
            ),
        ];
        for (label, list, detail) in cases {
            let message = RegistrySet::from_locations(list)
                .err()
                .expect(label)
                .to_string();
            assert!(message.contains(detail), "{label}: {message}");
        }
    }
}
