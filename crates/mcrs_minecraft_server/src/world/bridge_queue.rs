use std::collections::VecDeque;

use bevy_ecs::component::Component;

use crate::world::bus::{OutboundPlayerPacket, PacketPriority};

// Outbound depth thresholds shared by bridge_outbound and dispatch_encode.
// Keeping them here avoids a circular import and ensures both sides enforce
// the same limits.
pub const DEPTH_LIMIT: usize = 256;
pub const DEPTH_DRAIN_TARGET: usize = 192;

// Inbound rate-bucket baselines. A real throughput measurement pass may
// revise these values.
pub const INBOUND_BUCKET_CAP: u32 = 100;
pub const INBOUND_REFILL_PER_TICK: u32 = 20;
pub const INBOUND_KICK_OVERFLOW_TICKS: u8 = 3;

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

/// Per-connection inbound token-bucket rate limiter.
///
/// `consume_or_flag` returns `true` while packets are within budget and `false`
/// (incrementing `overflow_ticks`) when the bucket is empty. The caller kicks
/// the connection once `overflow_ticks >= INBOUND_KICK_OVERFLOW_TICKS`. `refill`
/// runs once per tick from `bridge_inbound` to restore tokens.
#[derive(Component)]
pub struct InboundRateBucket {
    tokens: u32,
    overflow_ticks: u8,
}

impl Default for InboundRateBucket {
    fn default() -> Self {
        Self::new()
    }
}

impl InboundRateBucket {
    pub fn new() -> Self {
        Self {
            tokens: INBOUND_BUCKET_CAP,
            overflow_ticks: 0,
        }
    }

    /// Attempt to consume one token. Returns `true` if the packet is within
    /// budget; returns `false` and increments `overflow_ticks` when the bucket
    /// is exhausted. Caller should kick the connection once
    /// `overflow_ticks >= INBOUND_KICK_OVERFLOW_TICKS`.
    pub fn consume_or_flag(&mut self) -> bool {
        if self.tokens > 0 {
            self.tokens -= 1;
            self.overflow_ticks = 0;
            true
        } else {
            self.overflow_ticks += 1;
            self.overflow_ticks < INBOUND_KICK_OVERFLOW_TICKS
        }
    }

    /// Refill the bucket by `INBOUND_REFILL_PER_TICK` tokens, capped at
    /// `INBOUND_BUCKET_CAP`. Call once per tick from `bridge_inbound`.
    pub fn refill(&mut self) {
        self.tokens = (self.tokens + INBOUND_REFILL_PER_TICK).min(INBOUND_BUCKET_CAP);
    }

    pub fn overflow_ticks(&self) -> u8 {
        self.overflow_ticks
    }
}
