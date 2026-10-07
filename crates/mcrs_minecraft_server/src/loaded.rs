use mcrs_minecraft_registry::{Entries, Registered, Registry, RegistrySet, Tags};

/// The registries the server cannot run without: the data pack loader builds each of them
/// before any system reads one, so an absent one is a broken loader, not a datapack error.
pub(crate) trait Loaded {
    fn loaded_registry<R: Registered>(&self) -> Registry<R>;
    fn loaded_tags<R: Registered>(&self) -> Tags<R>;
    fn loaded_entries<R: Registered, T: 'static>(&self) -> Entries<R, T>;
}

impl Loaded for RegistrySet {
    fn loaded_registry<R: Registered>(&self) -> Registry<R> {
        self.registry::<R>()
            .unwrap_or_else(|| panic!("the data pack loader declares {}", R::REGISTRY))
    }

    fn loaded_tags<R: Registered>(&self) -> Tags<R> {
        self.tags::<R>()
            .unwrap_or_else(|| panic!("the data pack loader builds the tags of {}", R::REGISTRY))
    }

    fn loaded_entries<R: Registered, T: 'static>(&self) -> Entries<R, T> {
        self.entries::<R, T>().unwrap_or_else(|| {
            panic!(
                "the data pack loader parses {} into {}",
                R::REGISTRY,
                std::any::type_name::<T>()
            )
        })
    }
}
