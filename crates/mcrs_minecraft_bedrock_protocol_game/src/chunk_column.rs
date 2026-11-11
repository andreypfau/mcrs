use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, NetworkChunkPos, VarU32};

use crate::dimension_id::DimensionId;

const REQUEST_ALL: u32 = 0xffff_ffff;
const REQUEST_UP_TO: u32 = 0xffff_fffe;

/// The blocks of one chunk column.
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x3a)]
pub struct ChunkColumn<'a> {
    /// The chunk position of the column.
    pub position: NetworkChunkPos,
    /// The dimension of the column.
    pub dimension: DimensionId,
    /// The sub-chunks in the payload, or the request mode of the client.
    pub sub_chunks: SubChunkCount,
    /// Whether the column is in blobs that the client caches; a server of mcrs writes
    /// [`BlobCache::Disabled`].
    pub blob_cache: BlobCache,
    /// The column data.
    pub payload: &'a [u8],
}

/// The sub-chunk count of [`ChunkColumn`]: a count, or one of two request modes. In a request mode
/// the payload holds no sub-chunk, and the client sends sub-chunk requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubChunkCount {
    /// The payload holds this number of sub-chunks. A count above `0xfffffffd` is an encode error.
    Included {
        /// The number of sub-chunks.
        count: u32,
    },
    /// The client requests all sub-chunks.
    RequestAll,
    /// The client requests the sub-chunks up to an index.
    RequestUpTo {
        /// The index of the highest sub-chunk that the client requests.
        highest: u16,
    },
}

impl BedrockEncode for SubChunkCount {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match *self {
            Self::Included { count } if count >= REQUEST_UP_TO => Err(Error::LengthOverflow),
            Self::Included { count } => VarU32(count).encode(w),
            Self::RequestAll => VarU32(REQUEST_ALL).encode(w),
            Self::RequestUpTo { highest } => {
                VarU32(REQUEST_UP_TO).encode(w)?;
                highest.encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for SubChunkCount {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(match VarU32::decode(r)? {
            VarU32(REQUEST_ALL) => Self::RequestAll,
            VarU32(REQUEST_UP_TO) => Self::RequestUpTo {
                highest: u16::decode(r)?,
            },
            VarU32(count) => Self::Included { count },
        })
    }
}

/// The blob cache flag of [`ChunkColumn`], with the blob ids when the cache is on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlobCache {
    /// The payload holds the whole column.
    Disabled,
    /// The column is in blobs that the client caches.
    Enabled {
        /// The hash of each blob, in the order of the parts of the column.
        blob_ids: Vec<u64>,
    },
}

impl BedrockEncode for BlobCache {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Disabled => false.encode(w),
            Self::Enabled { blob_ids } => {
                true.encode(w)?;
                blob_ids.as_slice().encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for BlobCache {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        if !bool::decode(r)? {
            return Ok(Self::Disabled);
        }
        Ok(Self::Enabled {
            blob_ids: BedrockDecode::decode(r)?,
        })
    }
}
