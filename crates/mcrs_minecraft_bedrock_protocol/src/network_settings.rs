use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

use crate::compression_algorithm::CompressionAlgorithm;

#[derive(Clone, Copy, Debug, PartialEq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x8f)]
pub struct NetworkSettings {
    pub threshold: u16,
    pub algorithm: CompressionAlgorithm,
    pub client_throttle_enabled: bool,
    pub client_throttle_threshold: u8,
    pub client_throttle_scalar: f32,
}
