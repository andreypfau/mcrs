use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::compression_algorithm::CompressionAlgorithm;
use crate::packet_header::{read_packet_header, write_packet_header};
use crate::packet_ids;

/// Server network settings response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkSettings {
    /// Minimum threshold in bytes before packets are compressed.
    pub compression_threshold: u16,
    /// Selected compression algorithm.
    pub compression_algorithm: CompressionAlgorithm,
    /// Client throttle enabled flag.
    pub client_throttle: bool,
    /// Client throttle threshold.
    pub client_throttle_threshold: u8,
    /// Client throttle scalar multiplier.
    pub client_throttle_scalar: f32,
}

impl BedrockEncode for NetworkSettings {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::NETWORK_SETTINGS, w)?;
        self.compression_threshold.encode(w)?;
        self.compression_algorithm.encode(w)?;
        self.client_throttle.encode(w)?;
        self.client_throttle_threshold.encode(w)?;
        self.client_throttle_scalar.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for NetworkSettings {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::NETWORK_SETTINGS)?;
        let compression_threshold = u16::decode(r)?;
        let compression_algorithm = CompressionAlgorithm::decode(r)?;
        let client_throttle = bool::decode(r)?;
        let client_throttle_threshold = u8::decode(r)?;
        let client_throttle_scalar = f32::decode(r)?;
        Ok(Self {
            compression_threshold,
            compression_algorithm,
            client_throttle,
            client_throttle_threshold,
            client_throttle_scalar,
        })
    }
}
