use core::convert::Infallible;
use core::fmt::Display;
use core::hash::Hash;
use core::marker::PhantomData;

use rustc_hash::FxHashMap;

use crate::disk_store::DiskStore;
use crate::versioned_chunk::{VersionedChunk, keep_latest};

/// A disk store that keeps its chunks in memory only, so the data layer of a world without a
/// save lives in memory. `E` is the error type that another disk of the same chunks shares.
#[derive(Debug)]
pub struct MemoryDisk<K, C, E = Infallible> {
    chunks: FxHashMap<K, VersionedChunk<C>>,
    error: PhantomData<fn() -> E>,
}

impl<K, C, E> MemoryDisk<K, C, E> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunks: FxHashMap::default(),
            error: PhantomData,
        }
    }
}

impl<K, C, E> Default for MemoryDisk<K, C, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, C, E> DiskStore for MemoryDisk<K, C, E>
where
    K: Copy + Eq + Hash + Send + 'static,
    C: Clone + Send + 'static,
    E: Display + Send + 'static,
{
    type Key = K;
    type Chunk = C;
    type Error = E;

    fn read(&mut self, key: K) -> Result<Option<VersionedChunk<C>>, E> {
        Ok(self.chunks.get(&key).cloned())
    }

    fn write(&mut self, key: K, chunk: VersionedChunk<C>) {
        keep_latest(&mut self.chunks, key, chunk);
    }
}
