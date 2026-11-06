use crate::versioned_chunk::VersionedChunk;

/// The answer of the data layer to a [`StoreInput::Get`](crate::StoreInput::Get). The answers
/// come in the order of the requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreOutput<K, C> {
    Found {
        key: K,
        chunk: VersionedChunk<C>,
    },
    /// Neither the cache nor the disk holds a version of `key`.
    Missing {
        key: K,
    },
    /// The disk did not read `key`. The stage logs the error.
    Unreadable {
        key: K,
    },
}
