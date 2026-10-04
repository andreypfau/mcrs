use crate::bitset::TagId;
use crate::dyn_index::DynRegistryIndex;
use crate::id::Id;
use crate::registry::Registry;
use crate::static_registry::{StaticId, StaticRegistry};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::tag_key::TaggedRegistry;

/// The registry a tag file's element references are resolved against.
pub trait TagSource: Send + Sync + 'static {
    type Id: TagId;

    fn id_of(&self, loc: &str) -> Option<Self::Id>;

    /// Upper bound on ids, used to size the frozen bitsets.
    fn capacity(&self) -> u32;
}

impl<T: Send + Sync + 'static> TagSource for StaticRegistry<T> {
    type Id = StaticId<T>;

    fn id_of(&self, loc: &str) -> Option<StaticId<T>> {
        StaticRegistry::id_of(self, loc)
    }

    fn capacity(&self) -> u32 {
        self.len() as u32
    }
}

impl<R: RegistryKey> TagSource for Registry<R> {
    type Id = Id<R>;

    fn id_of(&self, loc: &str) -> Option<Id<R>> {
        self.get(loc)
    }

    fn capacity(&self) -> u32 {
        self.len() as u32
    }
}

impl<T: TaggedRegistry + Send + Sync + 'static> TagSource for DynRegistryIndex<T> {
    type Id = u32;

    fn id_of(&self, loc: &str) -> Option<u32> {
        DynRegistryIndex::get(self, loc)
    }

    fn capacity(&self) -> u32 {
        self.len()
    }
}
