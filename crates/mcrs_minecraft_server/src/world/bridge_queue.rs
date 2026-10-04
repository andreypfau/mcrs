use std::collections::VecDeque;

use bevy_ecs::component::Component;
pub use mcrs_minecraft_network::inbound_rate::{
    INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_PACKETS, INBOUND_REFILL_PER_SECOND, InboundRateBucket,
};

use crate::world::bus::{OutboundPlayerPacket, PacketPriority};

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
    pub critical: VecDeque<OutboundPlayerPacket>,
    pub high: VecDeque<OutboundPlayerPacket>,
    pub normal: VecDeque<OutboundPlayerPacket>,
    pub low: VecDeque<OutboundPlayerPacket>,
}

impl OutboundQueue {
    pub fn push(&mut self, pkt: OutboundPlayerPacket) {
        match pkt.priority {
            PacketPriority::Critical => self.critical.push_back(pkt),
            PacketPriority::High => self.high.push_back(pkt),
            PacketPriority::Normal => self.normal.push_back(pkt),
            PacketPriority::Low => self.low.push_back(pkt),
        }
    }

    pub fn total_len(&self) -> usize {
        self.critical.len() + self.high.len() + self.normal.len() + self.low.len()
    }
}
