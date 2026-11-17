use core::fmt::Display;
use core::hash::Hash;

use crate::versioned_chunk::VersionedChunk;

pub trait DiskStore: Send + 'static {
    type Key: Copy + Eq + Hash + Send + 'static;
    type Chunk: Clone + Send + 'static;
    type Error: Display + Send + 'static;

    fn read(&mut self, key: Self::Key) -> Result<Option<VersionedChunk<Self::Chunk>>, Self::Error>;

    fn write(&mut self, key: Self::Key, chunk: VersionedChunk<Self::Chunk>);

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
