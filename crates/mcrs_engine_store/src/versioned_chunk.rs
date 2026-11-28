use core::hash::Hash;

use rustc_hash::FxHashMap;

use crate::chunk_version::ChunkVersion;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionedChunk<C> {
    pub version: ChunkVersion,
    pub chunk: C,
}

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
