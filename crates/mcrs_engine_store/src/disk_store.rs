use core::fmt::Display;
use core::hash::Hash;

use crate::versioned_chunk::VersionedChunk;

/// The store on disk under the cache of the data layer. The game implements it for its own
/// format of chunks on disk.
pub trait DiskStore: Send + 'static {
    type Key: Copy + Eq + Hash + Send + 'static;
    type Chunk: Clone + Send + 'static;
    type Error: Display + Send + 'static;

    /// The chunk at `key`, or `None` when the disk holds none.
    fn read(&mut self, key: Self::Key) -> Result<Option<VersionedChunk<Self::Chunk>>, Self::Error>;

    fn write(
        &mut self,
        key: Self::Key,
        chunk: &VersionedChunk<Self::Chunk>,
    ) -> Result<(), Self::Error>;
}
