use thiserror::Error;

use crate::compression_algorithm::CompressionAlgorithm;

/// An error of batch framing or of a packet codec.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    #[error("wire codec error: {0}")]
    Wire(#[from] mcrs_minecraft_bedrock_wire::Error),
    #[error("deflate stream error: {0}")]
    Deflate(#[from] std::io::Error),
    #[error("inflated batch exceeds {limit} bytes")]
    BatchTooLarge { limit: usize },
    #[error("packet length exceeds the rest of the batch")]
    TruncatedPacket,
    #[error("invalid batch header: expected 0xFE, got 0x{0:02X}")]
    InvalidBatchHeader(u8),
    #[error("batch packet is empty")]
    EmptyBatch,
    #[error("batch compression marker 0x{0:02X} differs from the negotiated algorithm")]
    UnexpectedCompressionMarker(u8),
    #[error("unsupported compression algorithm: {0:?}")]
    UnsupportedCompression(CompressionAlgorithm),
}
