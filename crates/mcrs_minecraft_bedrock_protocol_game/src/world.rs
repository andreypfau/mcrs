//! Bedrock Edition world state packets: SetTime, UpdateBlock, and ChunkColumn.

use mcrs_minecraft_bedrock_protocol::{read_packet_header, write_packet_header};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32, VarU32};

use crate::packet_ids;

/// Time of day update packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetTime {
    pub time: VarI32,
}

impl BedrockEncode for SetTime {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::SET_TIME, w)?;
        self.time.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for SetTime {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::SET_TIME)?;
        let time = VarI32::decode(r)?;
        Ok(Self { time })
    }
}

/// Block change update packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateBlock {
    pub x: VarI32,
    pub y: VarU32,
    pub z: VarI32,
    pub block_runtime_id: VarU32,
    pub flags: VarU32,
    pub layer: VarU32,
}

impl BedrockEncode for UpdateBlock {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::UPDATE_BLOCK, w)?;
        self.x.encode(w)?;
        self.y.encode(w)?;
        self.z.encode(w)?;
        self.block_runtime_id.encode(w)?;
        self.flags.encode(w)?;
        self.layer.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for UpdateBlock {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::UPDATE_BLOCK)?;
        let x = VarI32::decode(r)?;
        let y = VarU32::decode(r)?;
        let z = VarI32::decode(r)?;
        let block_runtime_id = VarU32::decode(r)?;
        let flags = VarU32::decode(r)?;
        let layer = VarU32::decode(r)?;
        Ok(Self {
            x,
            y,
            z,
            block_runtime_id,
            flags,
            layer,
        })
    }
}

/// Sub-chunk chunk payload packet for Bedrock Edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkColumn {
    pub chunk_x: VarI32,
    pub chunk_z: VarI32,
    pub dimension: VarI32,
    pub sub_chunk_count: VarU32,
    pub cache_enabled: bool,
    pub data: Vec<u8>,
}

impl BedrockEncode for ChunkColumn {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::CHUNK_COLUMN, w)?;
        self.chunk_x.encode(w)?;
        self.chunk_z.encode(w)?;
        self.dimension.encode(w)?;
        self.sub_chunk_count.encode(w)?;
        self.cache_enabled.encode(w)?;
        let len = u32::try_from(self.data.len()).map_err(|_| Error::LengthOverflow)?;
        VarU32(len).encode(w)?;
        w.extend_from_slice(&self.data);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for ChunkColumn {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::CHUNK_COLUMN)?;
        let chunk_x = VarI32::decode(r)?;
        let chunk_z = VarI32::decode(r)?;
        let dimension = VarI32::decode(r)?;
        let sub_chunk_count = VarU32::decode(r)?;
        let cache_enabled = bool::decode(r)?;
        let len = usize::try_from(VarU32::decode(r)?.0).map_err(|_| Error::LengthOverflow)?;
        if r.len() < len {
            return Err(Error::UnexpectedEof);
        }
        let (data_bytes, rest) = r.split_at(len);
        *r = rest;
        Ok(Self {
            chunk_x,
            chunk_z,
            dimension,
            sub_chunk_count,
            cache_enabled,
            data: data_bytes.to_vec(),
        })
    }
}
