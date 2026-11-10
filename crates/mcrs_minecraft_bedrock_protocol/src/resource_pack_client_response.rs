use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_header::{read_packet_header, write_packet_header};
use crate::packet_ids;

/// Status of the client response during resource pack negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum ResourcePackResponseStatus {
    None = 0,
    Refused = 1,
    SendPacks = 2,
    HaveAllPacks = 3,
    Completed = 4,
}

/// Client response to resource packs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePackClientResponse {
    pub response_status: ResourcePackResponseStatus,
    pub pack_ids: Vec<String>,
}

impl BedrockEncode for ResourcePackClientResponse {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::RESOURCE_PACK_CLIENT_RESPONSE, w)?;
        (self.response_status as u8).encode(w)?;
        let count = u16::try_from(self.pack_ids.len()).map_err(|_| Error::LengthOverflow)?;
        count.encode(w)?;
        for id in &self.pack_ids {
            id.encode(w)?;
        }
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for ResourcePackClientResponse {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::RESOURCE_PACK_CLIENT_RESPONSE)?;
        let response_status = match u8::decode(r)? {
            0 => ResourcePackResponseStatus::None,
            1 => ResourcePackResponseStatus::Refused,
            2 => ResourcePackResponseStatus::SendPacks,
            3 => ResourcePackResponseStatus::HaveAllPacks,
            4 => ResourcePackResponseStatus::Completed,
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };

        let count = usize::from(u16::decode(r)?);
        let mut pack_ids = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            pack_ids.push(String::decode(r)?);
        }

        Ok(Self {
            response_status,
            pack_ids,
        })
    }
}
