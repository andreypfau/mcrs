use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarU32};

use crate::packet_header::{decode_be_i32, encode_be_i32, read_packet_header, write_packet_header};
use crate::packet_ids;

/// The client login packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    /// Big-endian on the wire.
    pub protocol_version: i32,
    /// The connection request with its JWT chain.
    pub payload: Vec<u8>,
}

impl BedrockEncode for Login {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::LOGIN, w)?;
        encode_be_i32(self.protocol_version, w);
        let len = u32::try_from(self.payload.len()).map_err(|_| Error::LengthOverflow)?;
        VarU32(len).encode(w)?;
        w.extend_from_slice(&self.payload);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for Login {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::LOGIN)?;
        let protocol_version = decode_be_i32(r)?;
        let len = usize::try_from(VarU32::decode(r)?.0).map_err(|_| Error::LengthOverflow)?;
        let Some((payload, rest)) = r.split_at_checked(len) else {
            return Err(Error::UnexpectedEof);
        };
        *r = rest;
        Ok(Self {
            protocol_version,
            payload: payload.to_vec(),
        })
    }
}
