use std::collections::VecDeque;
use std::{iter, mem};

use bytes::{Bytes, BytesMut};
use mcrs_minecraft_protocol::{
    CompressionThreshold, Encode, Packet, PacketEncoder, RECORD_LENGTH_BYTES, write_record,
};
use tracing::warn;

use crate::packet_io::MAX_QUEUED_BYTES_PER_SOCKET;

#[derive(Debug)]
enum OutboundPart {
    Packets(Bytes),
    Shared(Bytes),
    Framed(Bytes),
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

const DEFERRED_BATCH_BYTES: usize = 64 * 1024;

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
    pub(crate) fn new(compression: CompressionThreshold) -> Self {
        Self {
            compression,
            ..Self::default()
        }
    }

    pub(crate) fn compression(&self) -> CompressionThreshold {
        self.compression
    }

    pub(crate) fn append<P: Packet + Encode>(&mut self, packet: &P) -> anyhow::Result<()> {
        write_record(&mut self.records, packet)?;
        if self.records.len() >= MAX_QUEUED_BYTES_PER_SOCKET {
            self.close_records();
        }
        Ok(())
    }

    pub(crate) fn append_shared(&mut self, packet: Bytes) {
        self.close_records();
        self.push(OutboundPart::Shared(packet));
    }

    pub(crate) fn append_records(&mut self, records: Bytes) -> usize {
        let count = packets(&records).count();
        self.close_records();
        self.push(OutboundPart::Packets(records));
        count
    }

    pub(crate) fn append_framed(&mut self, frame: Bytes) {
        self.close_records();
        self.push(OutboundPart::Framed(frame));
    }

    pub(crate) fn append_deferred(&mut self, records: Bytes) -> usize {
        let count = packets(&records).count();
        self.deferred_bytes = self.deferred_bytes.saturating_add(records.len());
        self.deferred.push_back(OutboundPart::Packets(records));
        count
    }

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

    pub(crate) fn close(&mut self) {
        self.close_records();
        if !self.open.parts.is_empty() {
            self.closed.push_back(mem::take(&mut self.open));
        }
    }

    pub(crate) fn pop(&mut self) -> Option<OutboundBatch> {
        if let Some(batch) = self.closed.pop_front() {
            return Some(batch);
        }
        let mut batch = OutboundBatch::default();
        while let Some(part) = self.deferred.pop_front() {
            if batch.bytes > 0 && batch.bytes.saturating_add(part.len()) > DEFERRED_BATCH_BYTES {
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

fn packets(records: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = records;
    iter::from_fn(move || {
        let (length, tail) = rest.split_first_chunk::<RECORD_LENGTH_BYTES>()?;
        let (packet, tail) = tail.split_at_checked(u32::from_be_bytes(*length) as usize)?;
        rest = tail;
        Some(packet)
    })
}

pub(crate) struct FrameWriter {
    encoder: PacketEncoder,
}

impl FrameWriter {
    pub(crate) fn new(encoder: PacketEncoder) -> Self {
        Self { encoder }
    }

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
    fn deferred_bytes_reach_the_writer_in_small_batches() {
        let mut queue = WriterQueue::default();
        let records = entity_records(0, 2000);
        for _ in 0..64 {
            queue.append_deferred(records.clone());
        }
        let first = queue.pop().unwrap();
        assert!(
            first.bytes() <= DEFERRED_BATCH_BYTES.max(records.len()),
            "a column written next waits behind at most one small batch"
        );
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
