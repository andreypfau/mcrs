use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bytes::Bytes;
use mcrs_minecraft_network::metrics::PacketKind;

use crate::bus::{OutboundPlayerPacket, PacketPayload};

/// The outbound packets of one connection, in the order the server wrote them.
///
/// The queue never reorders and never drops: the client applies the packets in wire order, and
/// the movement checks compare that order with the answers of the probes around the changes that
/// the client predicts with. A client that cannot take the packets is disconnected instead.
#[derive(Component, Default)]
pub struct OutboundQueue {
    packets: VecDeque<QueuedPacket>,
}

impl OutboundQueue {
    pub fn push(&mut self, pkt: OutboundPlayerPacket) {
        self.push_queued(QueuedPacket::from(pkt.data));
    }

    pub fn push_queued(&mut self, packet: QueuedPacket) {
        self.packets.push_back(packet);
    }

    pub fn total_len(&self) -> usize {
        self.packets.len()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &QueuedPacket> + ExactSizeIterator {
        self.packets.iter()
    }

    /// Takes every packet out, oldest first.
    pub fn drain(&mut self) -> impl Iterator<Item = QueuedPacket> + '_ {
        self.packets.drain(..)
    }
}

/// A packet that waits in an [`OutboundQueue`].
#[derive(Clone, Debug)]
pub enum QueuedPacket {
    /// Serialized when the queue of its one connection is handed over.
    Payload(PacketPayload),
    /// Serialized once for every connection that receives it.
    Encoded(EncodedPacket),
    /// Records of packets with their ids, from the muxer.
    Records(Vec<Bytes>),
}

/// A packet with its id, as `encode_with_id` wrote it.
#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub kind: PacketKind,
    /// Whether the payload it was serialized from changes the client's prediction, which the
    /// bytes no longer show.
    pub changes_prediction: bool,
    pub packet: Bytes,
}

/// Whether `payload` changes what the client predicts its own moves with, which decides whether
/// the probes bracket it: the blocks and columns the client collides with, its abilities, and
/// the motion, data and attributes of its own entity.
pub(crate) fn payload_changes_prediction(payload: &PacketPayload) -> bool {
    matches!(
        payload,
        PacketPayload::BlockUpdate(_)
            | PacketPayload::SectionBlocksUpdate(_)
            | PacketPayload::ChunkLoad { .. }
            | PacketPayload::ChunkUnload(_)
            | PacketPayload::PlayerAbilities(_)
            | PacketPayload::OwnEntityState(_)
    )
}

impl QueuedPacket {
    /// Whether the packet changes what the client predicts its own moves with.
    #[must_use]
    pub fn changes_prediction(&self) -> bool {
        match self {
            Self::Payload(payload) => payload_changes_prediction(payload),
            Self::Encoded(encoded) => encoded.changes_prediction,
            Self::Records(_) => false,
        }
    }

    pub fn kind(&self) -> PacketKind {
        match self {
            Self::Payload(payload) => payload.kind(),
            Self::Encoded(encoded) => encoded.kind,
            Self::Records(_) => PacketKind::Replication,
        }
    }
}

impl From<PacketPayload> for QueuedPacket {
    fn from(payload: PacketPayload) -> Self {
        let kind = payload.kind();
        let changes_prediction = payload_changes_prediction(&payload);
        match payload {
            PacketPayload::ChunkLoad { packet, .. } | PacketPayload::Commands(packet) => {
                Self::Encoded(EncodedPacket {
                    kind,
                    changes_prediction,
                    packet,
                })
            }
            PacketPayload::Replication(records) => Self::Records(records),
            payload => Self::Payload(payload),
        }
    }
}
