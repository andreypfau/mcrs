use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bytes::Bytes;
pub use mcrs_minecraft_network::inbound_rate::{
    INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_PACKETS, INBOUND_REFILL_PER_SECOND, InboundRateBucket,
};
use mcrs_minecraft_network::metrics::PacketKind;

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority};

// Outbound depth thresholds shared by bridge_outbound and dispatch_encode.
// Keeping them here avoids a circular import and ensures both sides enforce
// the same limits.
pub const DEPTH_LIMIT: usize = 256;
pub const DEPTH_DRAIN_TARGET: usize = 192;

/// Per-connection priority outbound queue.
///
/// Four sub-deques ordered Critical → High → Normal → Low. `dispatch_encode`
/// drains in that order and enforces `DEPTH_LIMIT`/`DEPTH_DRAIN_TARGET`
/// shedding on Normal/Low before flushing. Critical and High are never shed and
/// never counted against the connection: how many the server produced in a tick
/// says nothing about the client, which paces its own columns by acknowledging
/// each batch.
#[derive(Component, Default)]
pub struct OutboundQueue {
    pub critical: VecDeque<QueuedPacket>,
    pub high: VecDeque<QueuedPacket>,
    pub normal: VecDeque<QueuedPacket>,
    pub low: VecDeque<QueuedPacket>,
}

impl OutboundQueue {
    pub fn push(&mut self, pkt: OutboundPlayerPacket) {
        self.push_queued(pkt.priority, QueuedPacket::from(pkt.data));
    }

    pub fn push_queued(&mut self, priority: PacketPriority, packet: QueuedPacket) {
        match priority {
            PacketPriority::Critical => self.critical.push_back(packet),
            PacketPriority::High => self.high.push_back(packet),
            PacketPriority::Normal => self.normal.push_back(packet),
            PacketPriority::Low => self.low.push_back(packet),
        }
    }

    pub fn total_len(&self) -> usize {
        self.critical.len() + self.high.len() + self.normal.len() + self.low.len()
    }
}

/// A packet that waits in an [`OutboundQueue`].
#[derive(Clone, Debug)]
pub enum QueuedPacket {
    /// Serialized when the queue of its one connection is handed over.
    Payload(PacketPayload),
    /// Serialized once for every connection that receives it.
    Encoded(EncodedPacket),
}

/// A packet with its id, as `encode_with_id` wrote it.
#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub kind: PacketKind,
    pub packet: Bytes,
}

impl QueuedPacket {
    pub fn kind(&self) -> PacketKind {
        match self {
            Self::Payload(payload) => payload.kind(),
            Self::Encoded(encoded) => encoded.kind,
        }
    }
}

impl From<PacketPayload> for QueuedPacket {
    fn from(payload: PacketPayload) -> Self {
        let kind = payload.kind();
        match payload {
            PacketPayload::ChunkLoad { packet, .. } | PacketPayload::Commands(packet) => {
                Self::Encoded(EncodedPacket { kind, packet })
            }
            payload => Self::Payload(payload),
        }
    }
}
