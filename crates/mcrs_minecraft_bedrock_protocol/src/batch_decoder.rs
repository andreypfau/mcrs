use flate2::{Decompress, FlushDecompress, Status};
use snap::raw::{Decoder, decompress_len};

use crate::batch_encoder::{BATCH_BYTE, DEFLATE_MARKER, SNAPPY_MARKER, UNCOMPRESSED_MARKER};
use crate::packet_list::PacketList;
use crate::protocol_error::ProtocolError;

/// The default inflate limit: 10 MiB.
pub const DEFAULT_INFLATE_LIMIT: usize = 10 * 1024 * 1024;

const MIN_INFLATE_GROWTH: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    BeforeSwitch,
    AfterSwitch,
}

/// Reads batches into a buffer that it keeps between batches.
pub struct BatchDecoder {
    inflated: Vec<u8>,
    inflate: Decompress,
    snappy: Decoder,
    inflate_limit: usize,
    phase: Phase,
}

impl BatchDecoder {
    /// A decoder before the switch, with [`DEFAULT_INFLATE_LIMIT`].
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_inflate_limit(DEFAULT_INFLATE_LIMIT)
    }

    /// A decoder before the switch that rejects a compressed packet list larger than
    /// `inflate_limit` bytes.
    #[must_use]
    pub fn new_with_inflate_limit(inflate_limit: usize) -> Self {
        Self {
            inflated: Vec::new(),
            inflate: Decompress::new(false),
            snappy: Decoder::new(),
            inflate_limit,
            phase: Phase::BeforeSwitch,
        }
    }

    /// Makes the switch: from now on each batch body starts with a marker.
    pub fn switch(&mut self) {
        self.phase = Phase::AfterSwitch;
    }

    /// Checks one transport message and returns its packets. An empty message gives no
    /// packet: the receiver discards it.
    pub fn decode<'a>(&'a mut self, message: &'a [u8]) -> Result<PacketList<'a>, ProtocolError> {
        let Some((&first, body)) = message.split_first() else {
            return Ok(PacketList::EMPTY);
        };
        if first != BATCH_BYTE {
            return Err(ProtocolError::NotABatch(first));
        }
        if self.phase == Phase::BeforeSwitch {
            return PacketList::checked(body);
        }
        let Some((&marker, rest)) = body.split_first() else {
            return Err(ProtocolError::MissingMarker);
        };
        match marker {
            UNCOMPRESSED_MARKER => PacketList::checked(rest),
            DEFLATE_MARKER => {
                self.inflate_into_buffer(rest)?;
                PacketList::checked(&self.inflated)
            }
            SNAPPY_MARKER => {
                self.decompress_snappy_into_buffer(rest)?;
                PacketList::checked(&self.inflated)
            }
            other => Err(ProtocolError::UnknownMarker(other)),
        }
    }

    fn inflate_into_buffer(&mut self, compressed: &[u8]) -> Result<(), ProtocolError> {
        self.inflate.reset(false);
        self.inflated.clear();
        let mut consumed = 0;
        let mut written = 0;
        loop {
            if written == self.inflated.len() {
                if written >= self.inflate_limit {
                    return self.check_stream_end(compressed, consumed);
                }
                let growth = written.max(MIN_INFLATE_GROWTH);
                let target = written.saturating_add(growth).min(self.inflate_limit);
                self.inflated.resize(target, 0);
            }
            let (Some(input), Some(output)) =
                (compressed.get(consumed..), self.inflated.get_mut(written..))
            else {
                return Err(ProtocolError::CorruptDeflate);
            };
            let progress = inflate_step(&mut self.inflate, input, output)?;
            consumed += progress.read;
            written += progress.written;
            if progress.status == Status::StreamEnd {
                break;
            }
            if progress.read == 0 && progress.written == 0 {
                return Err(ProtocolError::CorruptDeflate);
            }
        }
        self.inflated.truncate(written);
        if consumed != compressed.len() {
            return Err(ProtocolError::CorruptDeflate);
        }
        Ok(())
    }

    fn decompress_snappy_into_buffer(&mut self, compressed: &[u8]) -> Result<(), ProtocolError> {
        let len = decompress_len(compressed).map_err(|_| ProtocolError::CorruptSnappy)?;
        if len > self.inflate_limit {
            return Err(ProtocolError::InflateLimitExceeded {
                limit: self.inflate_limit,
            });
        }
        self.inflated.clear();
        self.inflated.resize(len, 0);
        let written = self
            .snappy
            .decompress(compressed, &mut self.inflated)
            .map_err(|_| ProtocolError::CorruptSnappy)?;
        self.inflated.truncate(written);
        Ok(())
    }

    // The output holds exactly the limit; the batch is valid only when no more output remains.
    fn check_stream_end(
        &mut self,
        compressed: &[u8],
        consumed: usize,
    ) -> Result<(), ProtocolError> {
        let Some(input) = compressed.get(consumed..) else {
            return Err(ProtocolError::CorruptDeflate);
        };
        let mut probe = [0u8; 1];
        let progress = inflate_step(&mut self.inflate, input, &mut probe)?;
        if progress.written != 0 {
            return Err(ProtocolError::InflateLimitExceeded {
                limit: self.inflate_limit,
            });
        }
        if progress.status != Status::StreamEnd || consumed + progress.read != compressed.len() {
            return Err(ProtocolError::CorruptDeflate);
        }
        Ok(())
    }
}

impl Default for BatchDecoder {
    fn default() -> Self {
        Self::new()
    }
}

struct InflateProgress {
    status: Status,
    read: usize,
    written: usize,
}

fn inflate_step(
    inflate: &mut Decompress,
    input: &[u8],
    output: &mut [u8],
) -> Result<InflateProgress, ProtocolError> {
    let total_in = inflate.total_in();
    let total_out = inflate.total_out();
    let status = inflate
        .decompress(input, output, FlushDecompress::None)
        .map_err(|_| ProtocolError::CorruptDeflate)?;
    let read = usize::try_from(inflate.total_in() - total_in)
        .map_err(|_| ProtocolError::CorruptDeflate)?;
    let written = usize::try_from(inflate.total_out() - total_out)
        .map_err(|_| ProtocolError::CorruptDeflate)?;
    Ok(InflateProgress {
        status,
        read,
        written,
    })
}
