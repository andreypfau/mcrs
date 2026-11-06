use crate::chunk_version::ChunkVersion;

/// One version of the data of a chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionedChunk<C> {
    pub version: ChunkVersion,
    pub chunk: C,
}
