use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

use crate::compression_algorithm::CompressionAlgorithm;

/// Sets the compression of both directions; both sides make the switch after its batch.
#[derive(Clone, Copy, Debug, PartialEq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x8f)]
pub struct NetworkSettings {
    /// The smallest packet list in bytes that is compressed; 0 turns compression off.
    pub threshold: u16,
    /// The algorithm of a compressed packet list.
    pub algorithm: CompressionAlgorithm,
    /// A server writes false.
    pub client_throttle_enabled: bool,
    /// A server writes 0.
    pub client_throttle_threshold: u8,
    /// A server writes 0.0.
    pub client_throttle_scalar: f32,
}
