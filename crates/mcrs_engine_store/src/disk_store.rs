use core::fmt::Display;
use core::hash::Hash;

use crate::versioned_chunk::VersionedChunk;

/// The store on disk under the cache of the data layer. The game implements it for its own
/// format of chunks on disk.
pub trait DiskStore: Send + 'static {
    type Key: Copy + Eq + Hash + Send + 'static;
    type Chunk: Clone + Send + 'static;
    type Error: Display + Send + 'static;

    /// The chunk at `key`, or `None` when the disk holds none. A chunk that `write` took and
    /// `flush` did not make durable yet is read back.
    fn read(&mut self, key: Self::Key) -> Result<Option<VersionedChunk<Self::Chunk>>, Self::Error>;

    /// Takes `chunk` as the version of `key` to make durable at the next `flush`. The disk
    /// keeps a later version that it already holds instead.
    fn write(&mut self, key: Self::Key, chunk: VersionedChunk<Self::Chunk>);

    /// Makes every chunk written so far durable. A chunk that this did not make durable stays
    /// with the disk for the next flush.
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
