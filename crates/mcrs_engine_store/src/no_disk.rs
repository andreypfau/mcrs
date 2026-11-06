use core::convert::Infallible;
use core::hash::Hash;
use core::marker::PhantomData;

use crate::disk_store::DiskStore;
use crate::versioned_chunk::VersionedChunk;

/// A disk store that keeps nothing, so the data layer lives in memory only.
#[derive(Debug)]
pub struct NoDisk<K, C> {
    types: PhantomData<fn() -> (K, C)>,
}

impl<K, C> NoDisk<K, C> {
    #[must_use]
    pub fn new() -> Self {
        Self { types: PhantomData }
    }
}

impl<K, C> Default for NoDisk<K, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, C> DiskStore for NoDisk<K, C>
where
    K: Copy + Eq + Hash + Send + 'static,
    C: Clone + Send + 'static,
{
    type Key = K;
    type Chunk = C;
    type Error = Infallible;

    fn read(&mut self, _key: K) -> Result<Option<VersionedChunk<C>>, Infallible> {
        Ok(None)
    }

    fn write(&mut self, _key: K, _chunk: &VersionedChunk<C>) -> Result<(), Infallible> {
        Ok(())
    }
}
