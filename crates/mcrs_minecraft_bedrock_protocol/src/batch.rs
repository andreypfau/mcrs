use std::borrow::Cow;
use std::io::{Read, Write};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use mcrs_minecraft_bedrock_wire::{BedrockEncode, Error, VarU32};

use crate::compression_algorithm::CompressionAlgorithm;
use crate::error::ProtocolError;

/// The first byte of every batch.
pub const BATCH_PACKET_ID: u8 = 0xFE;

/// The compression state of a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchCompression {
    /// Before `NetworkSettings`: a batch has no compression marker.
    Pending,
    /// After `NetworkSettings`: a batch starts with a compression marker.
    Negotiated(CompressionAlgorithm),
}

/// Frames packet payloads as one batch.
///
/// A negotiated batch smaller than `threshold` goes out uncompressed.
pub fn encode_batch(
    packets: &[&[u8]],
    compression: BatchCompression,
    threshold: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let mut body = Vec::new();
    for packet in packets {
        let len = u32::try_from(packet.len()).map_err(|_| Error::LengthOverflow)?;
        VarU32(len).encode(&mut body)?;
        body.extend_from_slice(packet);
    }

    let mut out = vec![BATCH_PACKET_ID];
    match compression {
        BatchCompression::Pending => out.extend_from_slice(&body),
        BatchCompression::Negotiated(CompressionAlgorithm::Deflate) if body.len() >= threshold => {
            out.push(CompressionAlgorithm::Deflate.batch_marker());
            let mut encoder = DeflateEncoder::new(out, flate2::Compression::default());
            encoder.write_all(&body)?;
            out = encoder.finish()?;
        }
        BatchCompression::Negotiated(
            CompressionAlgorithm::Deflate | CompressionAlgorithm::None,
        ) => {
            out.push(CompressionAlgorithm::None.batch_marker());
            out.extend_from_slice(&body);
        }
        BatchCompression::Negotiated(algorithm) => {
            return Err(ProtocolError::UnsupportedCompression(algorithm));
        }
    }
    Ok(out)
}

/// Splits a batch into its packet payloads.
///
/// A compressed batch that inflates past `max_inflated_len` is an error.
pub fn decode_batch(
    batch: &[u8],
    compression: BatchCompression,
    max_inflated_len: usize,
) -> Result<Vec<Vec<u8>>, ProtocolError> {
    let Some((&header, payload)) = batch.split_first() else {
        return Err(ProtocolError::EmptyBatch);
    };
    if header != BATCH_PACKET_ID {
        return Err(ProtocolError::InvalidBatchHeader(header));
    }

    let body = match compression {
        BatchCompression::Pending => Cow::Borrowed(payload),
        BatchCompression::Negotiated(algorithm) => {
            let Some((&marker, rest)) = payload.split_first() else {
                return Err(ProtocolError::EmptyBatch);
            };
            match algorithm {
                _ if marker == CompressionAlgorithm::None.batch_marker() => Cow::Borrowed(rest),
                CompressionAlgorithm::Deflate if marker == algorithm.batch_marker() => {
                    Cow::Owned(inflate(rest, max_inflated_len)?)
                }
                CompressionAlgorithm::Snappy if marker == algorithm.batch_marker() => {
                    return Err(ProtocolError::UnsupportedCompression(algorithm));
                }
                _ => return Err(ProtocolError::UnexpectedCompressionMarker(marker)),
            }
        }
    };

    let mut r = body.as_ref();
    let mut packets = Vec::new();
    while !r.is_empty() {
        let len = usize::try_from(VarU32::decode(&mut r)?.0).map_err(|_| Error::LengthOverflow)?;
        let Some((packet, rest)) = r.split_at_checked(len) else {
            return Err(ProtocolError::TruncatedPacket);
        };
        packets.push(packet.to_vec());
        r = rest;
    }
    Ok(packets)
}

fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, ProtocolError> {
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut out = Vec::new();
    DeflateDecoder::new(data)
        .take(read_limit)
        .read_to_end(&mut out)?;
    if out.len() > limit {
        return Err(ProtocolError::BatchTooLarge { limit });
    }
    Ok(out)
}
