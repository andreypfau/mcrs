use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bytes::Bytes;
use mcrs_minecraft_network::metrics::PacketKind;

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority};

/// The outbound packets of one connection, in the order the server wrote them.
///
/// The queue never reorders: the client applies the packets in wire order, and the movement
/// checks compare that order with the answers of the probes around the changes that the client
/// predicts with. The priority only decides what a deep queue may shed: a Normal packet that no
/// prediction depends on. How many Critical or High packets the server produced in a tick says
/// nothing about the client, which paces its own columns by acknowledging each batch.
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

impl Queued {
    /// Whether a deep queue may drop the packet: a Normal packet that the client may miss.
    #[must_use]
    pub fn may_shed(&self) -> bool {
        self.priority == PacketPriority::Normal && !self.packet.must_arrive()
    }
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

    /// Drops the oldest packets that [`Queued::may_shed`] allows until `target` packets remain
    /// or none of the rest may be shed. Answers how many it dropped.
    pub fn shed_to(&mut self, target: usize) -> usize {
        let mut excess = self.packets.len().saturating_sub(target);
        let before = self.packets.len();
        self.packets.retain(|queued| {
            if excess > 0 && queued.may_shed() {
                excess -= 1;
                false
            } else {
                true
            }
        });
        before.saturating_sub(self.packets.len())
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
    pub packet: Bytes,
}

impl QueuedPacket {
    /// Whether the packet changes what the client predicts its own moves with: the blocks and
    /// columns it collides with, its abilities, and the motion, data and attributes of its own
    /// entity.
    #[must_use]
    pub fn changes_prediction(&self) -> bool {
        match self {
            Self::Payload(payload) => matches!(
                payload,
                PacketPayload::BlockUpdate(_)
                    | PacketPayload::SectionBlocksUpdate(_)
                    | PacketPayload::ChunkUnload(_)
                    | PacketPayload::PlayerAbilities(_)
                    | PacketPayload::OwnEntityState(_)
            ),
            Self::Encoded(encoded) => encoded.kind == PacketKind::Chunk,
            Self::Records(_) => false,
        }
    }

    /// Whether the client must get the packet: a change it predicts with, or a teleport, which
    /// it confirms.
    #[must_use]
    pub fn must_arrive(&self) -> bool {
        self.changes_prediction()
            || matches!(
                self,
                Self::Payload(
                    PacketPayload::PlayerPosition(_)
                        | PacketPayload::Respawn(_)
                        | PacketPayload::PlayerLogin(_)
                )
            )
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
        match payload {
            PacketPayload::ChunkLoad { packet, .. } | PacketPayload::Commands(packet) => {
                Self::Encoded(EncodedPacket { kind, packet })
            }
            PacketPayload::Replication(records) => Self::Records(records),
            payload => Self::Payload(payload),
        }
    }
}
