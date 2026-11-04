use core::hash::Hash;

use rustc_hash::FxHashMap;

use crate::disk_write::DiskWrite;
use crate::versioned_chunk::VersionedChunk;

/// The chunks of the data layer in memory, by key.
#[derive(Debug)]
pub struct ChunkCache<K, C> {
    entries: FxHashMap<K, CacheEntry<C>>,
}

#[derive(Debug)]
struct CacheEntry<C> {
    chunk: VersionedChunk<C>,
    disk: DiskState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiskState {
    Written,
    Unwritten,
}

impl<K, C> Default for ChunkCache<K, C> {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
        }
    }
}

impl<K: Copy + Eq + Hash, C> ChunkCache<K, C> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, key: K) -> Option<&VersionedChunk<C>> {
        self.entries.get(&key).map(|entry| &entry.chunk)
    }

    /// Keeps `chunk` as the version of `key` that the disk does not hold yet. It keeps a later
    /// version that it already holds instead, and tells whether it took `chunk`.
    pub fn put(&mut self, key: K, chunk: VersionedChunk<C>) -> bool {
        if self
            .entries
            .get(&key)
            .is_some_and(|held| held.chunk.version > chunk.version)
        {
            return false;
        }
        self.entries.insert(
            key,
            CacheEntry {
                chunk,
                disk: DiskState::Unwritten,
            },
        );
        true
    }

    /// Keeps `chunk`, read from the disk, as the version of `key`.
    pub fn insert_read(&mut self, key: K, chunk: VersionedChunk<C>) {
        self.entries.insert(
            key,
            CacheEntry {
                chunk,
                disk: DiskState::Written,
            },
        );
    }

    /// Hands each version that the disk does not hold yet to `write`, and counts it as written
    /// when `write` tells that it wrote it.
    pub fn write_unwritten(&mut self, mut write: impl FnMut(K, &VersionedChunk<C>) -> DiskWrite) {
        for (key, entry) in &mut self.entries {
            if entry.disk == DiskState::Unwritten && write(*key, &entry.chunk) == DiskWrite::Done {
                entry.disk = DiskState::Written;
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
