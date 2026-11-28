use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    #[error("wire codec error: {0}")]
    Wire(#[from] mcrs_minecraft_bedrock_wire::Error),
    #[error("transport message starts with {0:#04x}, not the batch byte 0xfe")]
    NotABatch(u8),
    #[error("batch body has no compression marker")]
    MissingMarker,
    #[error("unknown compression marker {0:#04x}")]
    UnknownMarker(u8),
    #[error("invalid Snappy block")]
    CorruptSnappy,
    #[error("invalid raw deflate stream")]
    CorruptDeflate,
    #[error("batch compression failed")]
    CompressFailed,
    #[error("packet list inflates past {limit} bytes")]
    InflateLimitExceeded { limit: usize },
    #[error("packet list has no entry")]
    EmptyPacketList,
    #[error("packet list entry has length 0")]
    EmptyEntry,
    #[error("packet list entry runs past the end of the list")]
    TruncatedEntry,
}
