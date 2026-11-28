use crate::compression_algorithm::CompressionAlgorithm;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BatchCompression {
    #[default]
    Pending,
    Negotiated {
        threshold: u16,
        algorithm: CompressionAlgorithm,
    },
}
