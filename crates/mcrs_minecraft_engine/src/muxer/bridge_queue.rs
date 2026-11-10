use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bytes::Bytes;
use mcrs_minecraft_network::metrics::PacketKind;

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority};

/// The outbound packets of one connection, in the order the server wrote them.
///
/// The queue never reorders and never drops: the client applies the packets in wire order, and
/// the movement checks compare that order with the answers of the probes around the changes that
/// the client predicts with. A client that cannot take the packets is disconnected instead.
#[derive(Component, Default)]
pub struct OutboundQueue {
    packets: VecDeque<Queued>,
}

/// A packet in an [`OutboundQueue`] with the priority it was written at.
#[derive(Clone, Debug)]
pub struct Queued {
    pub priority: PacketPriority,
    pub packet: QueuedPacket,
}

impl OutboundQueue {
    pub fn push(&mut self, pkt: OutboundPlayerPacket) {
        self.push_queued(pkt.priority, QueuedPacket::from(pkt.data));
    }

    pub fn push_queued(&mut self, priority: PacketPriority, packet: QueuedPacket) {
        self.packets.push_back(Queued { priority, packet });
    }

    pub fn total_len(&self) -> usize {
        self.packets.len()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Queued> + ExactSizeIterator {
        self.packets.iter()
    }

    /// Takes every packet out, oldest first.
    pub fn drain(&mut self) -> impl Iterator<Item = Queued> + '_ {
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
    /// The need of the payload it was serialized from, which the bytes no longer show.
    pub need: Need,
    pub packet: Bytes,
}

/// What the client needs a packet for, which decides whether the probes bracket it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// A change that the client predicts its own moves with.
    PredictedChange,
    /// A packet that the client does not predict with.
    Other,
}

impl Need {
    /// The need of `payload`. The predicted changes are the blocks and columns the client
    /// collides with, its abilities, and the motion, data and attributes of its own entity.
    #[must_use]
    pub fn of(payload: &PacketPayload) -> Self {
        match payload {
            PacketPayload::BlockUpdate(_)
            | PacketPayload::SectionBlocksUpdate(_)
            | PacketPayload::ChunkLoad { .. }
            | PacketPayload::ChunkUnload(_)
            | PacketPayload::PlayerAbilities(_)
            | PacketPayload::OwnEntityState(_) => Self::PredictedChange,
            _ => Self::Other,
        }
    }
}

impl QueuedPacket {
    /// What the client needs the packet for.
    #[must_use]
    pub fn need(&self) -> Need {
        match self {
            Self::Payload(payload) => Need::of(payload),
            Self::Encoded(encoded) => encoded.need,
            Self::Records(_) => Need::Other,
        }
    }

    /// Whether the packet changes what the client predicts its own moves with.
    #[must_use]
    pub fn changes_prediction(&self) -> bool {
        self.need() == Need::PredictedChange
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
        let need = Need::of(&payload);
        match payload {
            PacketPayload::ChunkLoad { packet, .. } | PacketPayload::Commands(packet) => {
                Self::Encoded(EncodedPacket { kind, need, packet })
            }
            PacketPayload::Replication(records) => Self::Records(records),
            payload => Self::Payload(payload),
        }
    }
}
