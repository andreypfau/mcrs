use crate::versioned_chunk::VersionedChunk;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreInput<K, C> {
    Put { key: K, chunk: VersionedChunk<C> },
    Get { key: K },
    Flush,
}
