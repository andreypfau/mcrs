use crate::names::NameTable;
use crate::registry::{Registry, RegistryError};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

type Tables = HashMap<ResourceLocation<Arc<str>>, Arc<NameTable>>;
type Paths = HashMap<Box<str>, Arc<NameTable>>;

#[derive(Clone, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct RegistrySet {
    tables: Arc<Tables>,
    paths: Arc<Paths>,
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
        Ok(Self::of(by_name))
    }

    fn of(tables: Tables) -> Self {
        let paths = tables
            .iter()
            .map(|(registry, table)| (registry.path().into(), Arc::clone(table)))
            .collect();
        RegistrySet {
            tables: Arc::new(tables),
            paths: Arc::new(paths),
        }
    }

    pub fn with<R: RegistryKey>(self, registry: Registry<R>) -> Result<Self, RegistryError> {
        if self.tables.contains_key(R::KEY.as_str()) {
            return Err(RegistryError::DuplicateRegistry {
                registry: R::KEY.into(),
            });
        }
        let mut tables = (*self.tables).clone();
        tables.insert(R::KEY.into(), Arc::clone(registry.table()));
        Ok(Self::of(tables))
    }

    pub fn registry<R: RegistryKey>(&self) -> Option<Registry<R>> {
        self.tables
            .get(R::KEY.as_str())
            .map(|table| Registry::view(Arc::clone(table)))
    }

    pub fn table(&self, registry: &str) -> Option<&Arc<NameTable>> {
        self.tables.get(registry)
    }

    pub(crate) fn table_at_path(&self, path: &str) -> Option<&NameTable> {
        self.paths.get(path).map(|table| &**table)
    }

    pub fn tables(&self) -> impl Iterator<Item = &Arc<NameTable>> {
        self.tables.values()
    }

    pub fn scope<T>(&self, run: impl FnOnce() -> T) -> T {
        struct Restore(Option<RegistrySet>);

        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                CURRENT.with_borrow_mut(|current| *current = previous);
            }
        }

        let previous = CURRENT.with_borrow_mut(|current| current.replace(self.clone()));
        let _restore = Restore(previous);
        run()
    }
}

pub(crate) fn current() -> Option<RegistrySet> {
    CURRENT.with_borrow(Clone::clone)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    NoScope {
        parsing: &'static str,
        registry: ResourceLocation<&'static str>,
    },
    MissingRegistry {
        parsing: &'static str,
        registry: ResourceLocation<&'static str>,
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
    use crate::id::Id;
    use mcrs_minecraft_core::rl;
    use serde::{Deserialize, Deserializer};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Barrier;

    struct Biome;

    impl RegistryKey for Biome {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:worldgen/biome");
    }

    struct Item;

    impl RegistryKey for Item {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:item");
    }

    const PLAINS_FIRST: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];
    const PLAINS_LAST: [&str; 3] = ["minecraft:desert", "minecraft:forest", "minecraft:plains"];

    fn set_of(names: &[&str]) -> RegistrySet {
        RegistrySet::new().with(registry::<Biome>(names)).unwrap()
    }

    fn parse(name: &str) -> Result<Id<Biome>, serde_json::Error> {
        serde_json::from_str(&format!("\"{name}\""))
    }

    fn index_of(name: &str) -> usize {
        parse(name).unwrap().index()
    }

    fn registry<R: RegistryKey>(names: &[&str]) -> Registry<R> {
        Registry::new(
            names
                .iter()
                .map(|text| ResourceLocation::parse(text).unwrap()),
            std::iter::empty(),
        )
        .unwrap()
    }

    fn no_scope_here() -> bool {
        Registry::<Biome>::in_scope("probe", |_| ())
            == Err(ScopeError::NoScope {
                parsing: "probe",
                registry: Biome::KEY,
            })
    }

    #[test]
    fn a_name_parses_to_its_id_inside_a_scope_and_is_written_back_as_the_name() {
        assert!(no_scope_here());
        let biomes = registry::<Biome>(&["minecraft:plains", "minecraft:desert"]);
        let set = RegistrySet::new().with(biomes.clone()).unwrap();
        set.scope(|| {
            let id: Id<Biome> = serde_json::from_str("\"minecraft:desert\"").unwrap();
            assert_eq!(Some(id), biomes.get("minecraft:desert"));
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
        assert!(message.contains("minecraft:worldgen/biome"), "{message}");
        let probe = Registry::<Biome>::in_scope("probe", |_| ());
        assert!(matches!(probe, Err(ScopeError::NoScope { .. })));
        assert!(no_scope_here());
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
                        assert!(message.contains("minecraft:worldgen/biome"), "{message}");
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
            .with(registry::<Item>(&["minecraft:stick"]))
            .unwrap();
        for set in [RegistrySet::new(), items_only] {
            set.scope(|| {
                let message = parse("minecraft:plains").unwrap_err().to_string();
                assert!(message.contains("Id<"), "{message}");
                assert!(message.contains("minecraft:worldgen/biome"), "{message}");
                let missing = Registry::<Biome>::in_scope("probe", |_| ()).unwrap_err();
                assert_eq!(
                    missing,
                    ScopeError::MissingRegistry {
                        parsing: "probe",
                        registry: Biome::KEY,
                    }
                );
                assert_ne!(
                    missing,
                    ScopeError::NoScope {
                        parsing: "probe",
                        registry: Biome::KEY,
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
            .with(registry::<Biome>(&PLAINS_FIRST))
            .unwrap();
        let error = set
            .clone()
            .with(registry::<Biome>(&PLAINS_LAST))
            .err()
            .unwrap();
        assert_eq!(
            error,
            RegistryError::DuplicateRegistry {
                registry: Biome::KEY.into()
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
}
