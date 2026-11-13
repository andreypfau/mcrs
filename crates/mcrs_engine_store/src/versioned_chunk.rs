use core::hash::Hash;

use rustc_hash::FxHashMap;

use crate::chunk_version::ChunkVersion;

/// One version of the data of a chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionedChunk<C> {
    pub version: ChunkVersion,
    pub chunk: C,
}

/// Keeps `chunk` as the version of `key`, unless `held` already holds a later version.
pub fn keep_latest<K: Eq + Hash, C>(
    held: &mut FxHashMap<K, VersionedChunk<C>>,
    key: K,
    chunk: VersionedChunk<C>,
) {
    if held
        .get(&key)
        .is_none_or(|current| current.version <= chunk.version)
    {
        held.insert(key, chunk);
    }
}
