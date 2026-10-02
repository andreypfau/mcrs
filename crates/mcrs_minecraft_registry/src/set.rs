use crate::registry::{Registry, RegistryError};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

type Registries = HashMap<ResourceLocation<&'static str>, Arc<dyn Any + Send + Sync>>;

#[derive(Clone, Default)]
pub struct RegistrySet {
    registries: Arc<Registries>,
}

thread_local! {
    static CURRENT: RefCell<Option<RegistrySet>> = const { RefCell::new(None) };
}

impl RegistrySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<R: RegistryKey>(self, registry: Registry<R>) -> Result<Self, RegistryError> {
        let mut registries = (*self.registries).clone();
        if registries.contains_key(&R::KEY) {
            return Err(RegistryError::DuplicateRegistry { registry: R::KEY });
        }
        registries.insert(R::KEY, Arc::new(registry));
        Ok(RegistrySet {
            registries: Arc::new(registries),
        })
    }

    pub fn registry<R: RegistryKey>(&self) -> Option<Registry<R>> {
        self.registries
            .get(&R::KEY)?
            .downcast_ref::<Registry<R>>()
            .cloned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use mcrs_minecraft_core::rl;

    struct Biome;

    impl RegistryKey for Biome {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:worldgen/biome");
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
}
