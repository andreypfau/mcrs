use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_header::{decode_be_i32, encode_be_i32, read_packet_header, write_packet_header};
use crate::packet_ids;

/// The first client packet: it asks for the network settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestNetworkSettings {
    /// Big-endian on the wire.
    pub client_protocol_version: i32,
}

impl BedrockEncode for RequestNetworkSettings {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::REQUEST_NETWORK_SETTINGS, w)?;
        encode_be_i32(self.client_protocol_version, w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for RequestNetworkSettings {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::REQUEST_NETWORK_SETTINGS)?;
        Ok(Self {
            client_protocol_version: decode_be_i32(r)?,
        })
    }
}
