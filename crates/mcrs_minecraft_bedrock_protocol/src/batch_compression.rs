use crate::compression_algorithm::CompressionAlgorithm;

/// The compression state of one direction of a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BatchCompression {
    /// Before the switch: a batch body is the packet list, with no marker.
    #[default]
    Pending,
    /// After the switch: a batch body starts with a marker.
    Negotiated {
        /// The smallest packet list in bytes that is compressed; 0 turns compression off.
        threshold: u16,
        /// The algorithm of a compressed packet list.
        algorithm: CompressionAlgorithm,
    },
}
