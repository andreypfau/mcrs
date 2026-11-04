use crate::versioned_chunk::VersionedChunk;

/// What a part of the engine asks of the data layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreInput<K, C> {
    /// Keeps `chunk` as the version of `key`, unless the data layer already holds a later one.
    Put { key: K, chunk: VersionedChunk<C> },
    /// Asks for the last version of `key`. The answer is a [`StoreOutput`](crate::StoreOutput).
    Get { key: K },
    /// Writes to the disk every version that the disk does not hold yet.
    Flush,
}
