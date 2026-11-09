use std::collections::VecDeque;
use std::{iter, mem};

use bytes::{Bytes, BytesMut};
use mcrs_minecraft_protocol::{
    CompressionThreshold, Encode, Packet, PacketEncoder, RECORD_LENGTH_BYTES, write_record,
};
use tracing::warn;

use crate::packet_io::MAX_QUEUED_BYTES_PER_SOCKET;

/// A piece of what a connection hands its writer, which frames and compresses the pieces in
/// order.
#[derive(Debug)]
enum OutboundPart {
    /// Packets with their ids, each behind its length as a big-endian `u32`.
    Packets(Bytes),
    /// One packet with its id, in a buffer that other connections share.
    Shared(Bytes),
    /// Frames made for the threshold the writer is at, which it sends as they are.
    Framed(Bytes),
    /// The writer compresses every packet after this one at the threshold.
    Compression(CompressionThreshold),
}

impl OutboundPart {
    fn len(&self) -> usize {
        match self {
            Self::Packets(bytes) | Self::Shared(bytes) | Self::Framed(bytes) => bytes.len(),
            Self::Compression(_) => 0,
        }
    }
}

/// One hand-over to the writer. A batch holds about [`MAX_QUEUED_BYTES_PER_SOCKET`] of packets
/// at most, so the writer's channel bounds what is in flight.
#[derive(Debug, Default)]
pub(crate) struct OutboundBatch {
    parts: Vec<OutboundPart>,
    bytes: usize,
}

impl OutboundBatch {
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    fn push(&mut self, part: OutboundPart) {
        self.bytes = self.bytes.saturating_add(part.len());
        self.parts.push(part);
    }
}

/// What a connection has written and its writer has not taken yet.
///
/// The server thread only serializes packets here; framing and compression wait for the
/// writer, so a packet's frame always matches the threshold the writer switched to before it.
///
/// The queue has two lanes. The ordered lane keeps the order of everything written to it. The
/// deferred lane holds the entity bytes of the replication, in their own order, and the writer
/// takes them only when the ordered lane is empty. Thus a client that reads slowly still gets
/// its columns, its teleports and its keep-alives first. A fence puts the deferred bytes back in
/// order, before a packet after which they no longer apply.
#[derive(Default)]
pub(crate) struct WriterQueue {
    records: BytesMut,
    open: OutboundBatch,
    closed: VecDeque<OutboundBatch>,
    deferred: VecDeque<OutboundPart>,
    deferred_bytes: usize,
    compression: CompressionThreshold,
}

impl WriterQueue {
    /// A queue for a writer that starts at `compression`.
    pub(crate) fn new(compression: CompressionThreshold) -> Self {
        Self {
            compression,
            ..Self::default()
        }
    }

    /// The threshold the writer frames the next appended packet at.
    pub(crate) fn compression(&self) -> CompressionThreshold {
        self.compression
    }

    /// A packet that fails to encode leaves nothing behind.
    pub(crate) fn append<P: Packet + Encode>(&mut self, packet: &P) -> anyhow::Result<()> {
        write_record(&mut self.records, packet)?;
        if self.records.len() >= MAX_QUEUED_BYTES_PER_SOCKET {
            self.close_records();
        }
        Ok(())
    }

    /// Queues a packet that `encode_with_id` wrote once for every connection that receives it.
    pub(crate) fn append_shared(&mut self, packet: Bytes) {
        self.close_records();
        self.push(OutboundPart::Shared(packet));
    }

    /// Queues packets that other connections may share, each behind its length as a big-endian
    /// `u32`, as [`Self::append`] writes them. Returns how many packets `records` holds.
    pub(crate) fn append_records(&mut self, records: Bytes) -> usize {
        let count = packets(&records).count();
        self.close_records();
        self.push(OutboundPart::Packets(records));
        count
    }

    /// Queues frames made at [`Self::compression`].
    pub(crate) fn append_framed(&mut self, frame: Bytes) {
        self.close_records();
        self.push(OutboundPart::Framed(frame));
    }

    /// Queues records as [`Self::append_records`] does, in the deferred lane.
    pub(crate) fn append_deferred(&mut self, records: Bytes) -> usize {
        let count = packets(&records).count();
        self.deferred_bytes = self.deferred_bytes.saturating_add(records.len());
        self.deferred.push_back(OutboundPart::Packets(records));
        count
    }

    /// Moves the deferred bytes into the ordered lane, ahead of what comes next.
    pub(crate) fn fence(&mut self) {
        self.close_records();
        while let Some(part) = self.deferred.pop_front() {
            self.push(part);
        }
        self.deferred_bytes = 0;
    }

    pub(crate) fn switch_compression(&mut self, threshold: CompressionThreshold) {
        self.fence();
        self.compression = threshold;
        self.close_records();
        self.push(OutboundPart::Compression(threshold));
    }

    /// Makes everything written so far ready for the writer.
    pub(crate) fn close(&mut self) {
        self.close_records();
        if !self.open.parts.is_empty() {
            self.closed.push_back(mem::take(&mut self.open));
        }
    }

    /// The next batch for the writer: the ordered lane first, then the deferred bytes.
    pub(crate) fn pop(&mut self) -> Option<OutboundBatch> {
        if let Some(batch) = self.closed.pop_front() {
            return Some(batch);
        }
        let mut batch = OutboundBatch::default();
        while let Some(part) = self.deferred.pop_front() {
            if batch.bytes > 0
                && batch.bytes.saturating_add(part.len()) > MAX_QUEUED_BYTES_PER_SOCKET
            {
                self.deferred.push_front(part);
                break;
            }
            self.deferred_bytes = self.deferred_bytes.saturating_sub(part.len());
            batch.push(part);
        }
        (!batch.parts.is_empty()).then_some(batch)
    }

    pub(crate) fn push_front(&mut self, batch: OutboundBatch) {
        self.closed.push_front(batch);
    }

    /// The bytes of the batches ready for the writer that it has not taken, with the deferred
    /// bytes.
    pub(crate) fn closed_bytes(&self) -> usize {
        self.closed
            .iter()
            .fold(self.deferred_bytes, |total, batch| {
                total.saturating_add(batch.bytes())
            })
    }

    fn close_records(&mut self) {
        if !self.records.is_empty() {
            let records = self.records.split().freeze();
            self.push(OutboundPart::Packets(records));
        }
    }

    fn push(&mut self, part: OutboundPart) {
        if self.open.bytes > 0
            && self.open.bytes.saturating_add(part.len()) > MAX_QUEUED_BYTES_PER_SOCKET
        {
            self.closed.push_back(mem::take(&mut self.open));
        }
        self.open.push(part);
    }
}

/// The packets of `records`, each behind its length as a big-endian `u32`. A record cut short
/// ends them.
fn packets(records: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = records;
    iter::from_fn(move || {
        let (length, tail) = rest.split_first_chunk::<RECORD_LENGTH_BYTES>()?;
        let (packet, tail) = tail.split_at_checked(u32::from_be_bytes(*length) as usize)?;
        rest = tail;
        Some(packet)
    })
}

/// The writer's half of the encoding: frames and compresses each batch it is handed, in order,
/// except the frames the server thread made once for many connections.
pub(crate) struct FrameWriter {
    encoder: PacketEncoder,
}

impl FrameWriter {
    /// Takes over the encoder of the handshake, with its threshold and any frames it holds.
    pub(crate) fn new(encoder: PacketEncoder) -> Self {
        Self { encoder }
    }

    /// Frames `batch` into `chunks`, to write in order. A frame the server thread made goes as
    /// the bytes the connections share, never copied.
    pub(crate) fn frame_into(&mut self, batch: OutboundBatch, chunks: &mut Vec<Bytes>) {
        for part in batch.parts {
            match part {
                OutboundPart::Packets(records) => {
                    for packet in packets(&records) {
                        self.append(packet);
                    }
                }
                OutboundPart::Shared(packet) => self.append(&packet),
                OutboundPart::Framed(frame) => {
                    self.take_into(chunks);
                    chunks.push(frame);
                }
                OutboundPart::Compression(threshold) => self.encoder.set_compression(threshold),
            }
        }
        self.take_into(chunks);
    }

    fn take_into(&mut self, chunks: &mut Vec<Bytes>) {
        let framed = self.encoder.take();
        if !framed.is_empty() {
            chunks.push(framed.freeze());
        }
    }

    fn append(&mut self, packet: &[u8]) {
        if let Err(error) = self.encoder.append_encoded(packet) {
            warn!("a packet could not be framed: {error:#}");
        }
    }
}

/// The far side of a connection made by [`crate::RawConnection::new_for_test_full`]: it frames
/// what the connection hands its writer, as the writer would.
#[cfg(not(target_family = "wasm"))]
pub struct OutboundReceiver {
    batches: tokio::sync::mpsc::Receiver<OutboundBatch>,
    frames: FrameWriter,
    chunks: Vec<Bytes>,
}

#[cfg(not(target_family = "wasm"))]
impl OutboundReceiver {
    pub(crate) fn new(batches: tokio::sync::mpsc::Receiver<OutboundBatch>) -> Self {
        Self {
            batches,
            frames: FrameWriter::new(PacketEncoder::new()),
            chunks: Vec::new(),
        }
    }

    /// The frames of the next hand-over that carries any. A hand-over that only switches the
    /// compression is applied and skipped.
    pub fn try_recv(&mut self) -> Result<Bytes, tokio::sync::mpsc::error::TryRecvError> {
        loop {
            let batch = self.batches.try_recv()?;
            self.frames.frame_into(batch, &mut self.chunks);
            if !self.chunks.is_empty() {
                return Ok(self.chunks.drain(..).flatten().collect());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_protocol::packets::common::clientbound::KeepAlive;
    use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;

    use super::*;

    fn keep_alive(payload: i64) -> ClientboundKeepAlive {
        ClientboundKeepAlive(KeepAlive { payload })
    }

    /// Records of `count` packets, which stand for the entity bytes of the replication.
    fn entity_records(first: i64, count: i64) -> Bytes {
        let mut records = BytesMut::new();
        for payload in first..first + count {
            write_record(&mut records, &keep_alive(payload)).unwrap();
        }
        records.freeze()
    }

    fn ordered_record(payload: i64) -> Bytes {
        let mut record = BytesMut::new();
        write_record(&mut record, &keep_alive(payload)).unwrap();
        record.freeze()
    }

    fn contents(batch: &OutboundBatch) -> Vec<Bytes> {
        batch
            .parts
            .iter()
            .filter_map(|part| match part {
                OutboundPart::Packets(bytes) => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_packet_written_after_deferred_entity_bytes_reaches_the_writer_first() {
        let mut queue = WriterQueue::default();
        let entities = entity_records(0, 1000);
        queue.append_deferred(entities.clone());
        queue.append(&keep_alive(-1)).unwrap();
        queue.close();

        assert_eq!(contents(&queue.pop().unwrap()), vec![ordered_record(-1)]);
        assert_eq!(contents(&queue.pop().unwrap()), vec![entities]);
        assert!(queue.pop().is_none());
    }

    #[test]
    fn a_fence_keeps_the_deferred_bytes_before_the_next_packet() {
        let mut queue = WriterQueue::default();
        let entities = entity_records(0, 10);
        queue.append_deferred(entities.clone());
        queue.fence();
        queue.append(&keep_alive(-1)).unwrap();
        queue.close();

        assert_eq!(
            contents(&queue.pop().unwrap()),
            vec![entities, ordered_record(-1)]
        );
        assert!(queue.pop().is_none());
    }
}
