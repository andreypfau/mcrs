use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{Registries, Registry, RegistryOf, RegistryType, Tags};
use std::collections::BTreeMap;

/// The registries the server cannot run without: the data pack loader builds each of them
/// before any system reads one, so an absent one is a broken loader, not a datapack error.
pub(crate) trait Loaded {
    fn loaded<V: Send + Sync + 'static>(&self) -> &Registry<RegistryOf<V>>;
    fn loaded_tags<R: RegistryType>(&self) -> Tags<R>;
    fn loaded_by_name<V: Clone + Send + Sync + 'static>(&self) -> BTreeMap<ResourceLocation, V>;
}

impl Loaded for Registries {
    fn loaded<V: Send + Sync + 'static>(&self) -> &Registry<RegistryOf<V>> {
        self.registry::<RegistryOf<V>>().unwrap_or_else(|| {
            panic!(
                "the data pack loader declares the registry of {}",
                std::any::type_name::<V>()
            )
        })
    }

    fn loaded_tags<R: RegistryType>(&self) -> Tags<R> {
        self.tags::<R>().unwrap_or_else(|| {
            panic!(
                "the data pack loader builds the tags of {}",
                std::any::type_name::<R>()
            )
        })
    }

    fn loaded_by_name<V: Clone + Send + Sync + 'static>(&self) -> BTreeMap<ResourceLocation, V> {
        self.loaded::<V>()
            .iter()
            .map(|(_, name, value)| (name.clone(), value.clone()))
            .collect()
    }
}
