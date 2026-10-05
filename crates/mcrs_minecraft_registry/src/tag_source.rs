use crate::bitset::DenseId;
use crate::dyn_index::DynRegistryIndex;
use crate::id::Id;
use crate::registry::Registry;
use mcrs_minecraft_core::registry_key::RegistryKey;

/// The registry a tag file's element references are resolved against.
pub trait TagSource: Send + Sync + 'static {
    type Id: DenseId;

    fn id_of(&self, loc: &str) -> Option<Self::Id>;

    /// Upper bound on ids, used to size the frozen bitsets.
    fn capacity(&self) -> u32;
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

impl<T: RegistryKey + Send + Sync + 'static> TagSource for DynRegistryIndex<T> {
    type Id = u16;

    fn id_of(&self, loc: &str) -> Option<u16> {
        DynRegistryIndex::get(self, loc)
    }

    fn capacity(&self) -> u32 {
        self.len()
    }
}
