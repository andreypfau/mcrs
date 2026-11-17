use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, NetworkChunkPos, VarU32};

use crate::dimension_id::DimensionId;

const REQUEST_ALL: u32 = 0xffff_ffff;
const REQUEST_UP_TO: u32 = 0xffff_fffe;

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x3a)]
pub struct ChunkColumn<'a> {
    pub position: NetworkChunkPos,
    pub dimension: DimensionId,
    pub sub_chunks: SubChunkCount,
    pub blob_cache: BlobCache,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubChunkCount {
    Included { count: u32 },
    RequestAll,
    RequestUpTo { highest: u16 },
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlobCache {
    Disabled,
    Enabled { blob_ids: Vec<u64> },
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
