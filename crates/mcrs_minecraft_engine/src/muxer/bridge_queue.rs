use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bytes::Bytes;
use mcrs_minecraft_network::metrics::PacketKind;

use crate::bus::{OutboundPlayerPacket, PacketPayload};

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

    pub fn drain(&mut self) -> impl Iterator<Item = QueuedPacket> + '_ {
        self.packets.drain(..)
    }
}

#[derive(Clone, Debug)]
pub enum QueuedPacket {
    Payload(PacketPayload),
    Encoded(EncodedPacket),
    Records(Vec<Bytes>),
}

#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub kind: PacketKind,
    pub changes_prediction: bool,
    pub packet: Bytes,
}

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
