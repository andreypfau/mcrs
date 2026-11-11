use flate2::{Compress, Compression, FlushCompress, Status};
use mcrs_minecraft_bedrock_wire::{BedrockEncode, Error, Packet, VarU32, encode_packet};
use snap::raw::{Encoder, max_compress_len};

use crate::batch_compression::BatchCompression;
use crate::compression_algorithm::CompressionAlgorithm;
use crate::protocol_error::ProtocolError;

pub(crate) const BATCH_BYTE: u8 = 0xfe;
pub(crate) const DEFLATE_MARKER: u8 = 0x00;
pub(crate) const SNAPPY_MARKER: u8 = 0x01;
pub(crate) const UNCOMPRESSED_MARKER: u8 = 0xff;

const DEFLATE_SLACK: usize = 64;

/// Builds batches into buffers that it keeps between batches.
///
/// Append the packets of one batch, then call [`BatchEncoder::finish`] for the transport
/// message.
pub struct BatchEncoder {
    packet_list: Vec<u8>,
    message: Vec<u8>,
    deflate: Compress,
    snappy: Encoder,
    compression: BatchCompression,
}

impl BatchEncoder {
    /// An encoder before the switch, with raw deflate at the default level.
    #[must_use]
    pub fn new() -> Self {
        Self {
            packet_list: Vec::new(),
            message: Vec::new(),
            deflate: Compress::new(Compression::default(), false),
            snappy: Encoder::new(),
            compression: BatchCompression::Pending,
        }
    }

    /// Sets the compression; the server calls it directly after it finishes the batch of
    /// `NetworkSettings`.
    pub fn set_compression(&mut self, compression: BatchCompression) {
        self.compression = compression;
    }

    /// Appends `packet` to the current batch as one entry of the packet list.
    pub fn append<P: Packet + BedrockEncode>(&mut self, packet: &P) -> Result<(), ProtocolError> {
        let start = self.packet_list.len();
        if let Err(error) = self.append_entry(start, packet) {
            self.packet_list.truncate(start);
            return Err(error.into());
        }
        Ok(())
    }

    fn append_entry<P: Packet + BedrockEncode>(
        &mut self,
        start: usize,
        packet: &P,
    ) -> Result<(), Error> {
        encode_packet(packet, &mut self.packet_list)?;
        let entry_len = self.packet_list.len() - start;
        let entry_len = u32::try_from(entry_len).map_err(|_| Error::LengthOverflow)?;
        VarU32(entry_len).encode(&mut self.packet_list)?;
        let prefix_len = self.packet_list.len() - start - entry_len as usize;
        if let Some(entry) = self.packet_list.get_mut(start..) {
            entry.rotate_right(prefix_len);
        }
        Ok(())
    }

    /// Ends the current batch and returns its transport message, which starts with `0xfe`.
    /// The next append starts a new batch. On an error the batch keeps its packets.
    pub fn finish(&mut self) -> Result<&[u8], ProtocolError> {
        if self.packet_list.is_empty() {
            return Err(ProtocolError::EmptyPacketList);
        }
        self.message.clear();
        self.message.push(BATCH_BYTE);
        self.write_body()?;
        self.packet_list.clear();
        Ok(&self.message)
    }

    fn write_body(&mut self) -> Result<(), ProtocolError> {
        let BatchCompression::Negotiated {
            threshold,
            algorithm,
        } = self.compression
        else {
            self.message.extend_from_slice(&self.packet_list);
            return Ok(());
        };
        if threshold == 0 || self.packet_list.len() < usize::from(threshold) {
            return self.write_uncompressed();
        }
        match algorithm {
            CompressionAlgorithm::Disabled => self.write_uncompressed(),
            CompressionAlgorithm::Deflate => self.write_deflate(),
            CompressionAlgorithm::Snappy => self.write_snappy(),
        }
    }

    fn write_uncompressed(&mut self) -> Result<(), ProtocolError> {
        self.message.push(UNCOMPRESSED_MARKER);
        self.message.extend_from_slice(&self.packet_list);
        Ok(())
    }

    fn write_snappy(&mut self) -> Result<(), ProtocolError> {
        self.message.push(SNAPPY_MARKER);
        let start = self.message.len();
        let end = start
            .checked_add(max_compress_len(self.packet_list.len()))
            .ok_or(ProtocolError::CompressFailed)?;
        self.message.resize(end, 0);
        let Some(output) = self.message.get_mut(start..) else {
            return Err(ProtocolError::CompressFailed);
        };
        let written = self
            .snappy
            .compress(&self.packet_list, output)
            .map_err(|_| ProtocolError::CompressFailed)?;
        self.message.truncate(start + written);
        Ok(())
    }

    fn write_deflate(&mut self) -> Result<(), ProtocolError> {
        self.message.push(DEFLATE_MARKER);
        self.deflate.reset();
        let mut consumed = 0;
        loop {
            let Some(input) = self.packet_list.get(consumed..) else {
                return Err(ProtocolError::CompressFailed);
            };
            self.message.reserve(input.len() + DEFLATE_SLACK);
            let before = self.deflate.total_in();
            let status = self
                .deflate
                .compress_vec(input, &mut self.message, FlushCompress::Finish)
                .map_err(|_| ProtocolError::CompressFailed)?;
            let read = usize::try_from(self.deflate.total_in() - before)
                .map_err(|_| ProtocolError::CompressFailed)?;
            consumed += read;
            if status == Status::StreamEnd {
                return Ok(());
            }
        }
    }
}

impl Default for BatchEncoder {
    fn default() -> Self {
        Self::new()
    }
}
