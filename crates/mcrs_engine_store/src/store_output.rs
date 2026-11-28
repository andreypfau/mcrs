use crate::versioned_chunk::VersionedChunk;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreOutput<K, C> {
    Found { key: K, chunk: VersionedChunk<C> },
    Missing { key: K },
    Unreadable { key: K },
}
