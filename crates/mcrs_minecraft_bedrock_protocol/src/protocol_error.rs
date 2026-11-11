use thiserror::Error;

/// A batch error or a packet codec error. A receiver handles no packet of a batch that
/// gives one of these, and a server closes the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// A packet or an entry length does not encode or decode.
    #[error("wire codec error: {0}")]
    Wire(#[from] mcrs_minecraft_bedrock_wire::Error),
    /// The transport message does not start with the batch byte `0xfe`.
    #[error("transport message starts with {0:#04x}, not the batch byte 0xfe")]
    NotABatch(u8),
    /// A batch body after the switch has no marker byte.
    #[error("batch body has no compression marker")]
    MissingMarker,
    /// The marker is not `0x00`, `0x01` or `0xff`.
    #[error("unknown compression marker {0:#04x}")]
    UnknownMarker(u8),
    /// The Snappy block is not valid or does not match its uncompressed size.
    #[error("invalid Snappy block")]
    CorruptSnappy,
    /// The raw deflate stream is not valid, does not end, or has bytes after its final block.
    #[error("invalid raw deflate stream")]
    CorruptDeflate,
    /// The deflate or Snappy compressor failed.
    #[error("batch compression failed")]
    CompressFailed,
    /// The packet list inflates to more than the inflate limit.
    #[error("packet list inflates past {limit} bytes")]
    InflateLimitExceeded {
        /// The inflate limit in bytes.
        limit: usize,
    },
    /// The packet list has no entry.
    #[error("packet list has no entry")]
    EmptyPacketList,
    /// An entry of the packet list has the length 0.
    #[error("packet list entry has length 0")]
    EmptyEntry,
    /// An entry length is larger than the bytes that remain in the packet list.
    #[error("packet list entry runs past the end of the list")]
    TruncatedEntry,
}
