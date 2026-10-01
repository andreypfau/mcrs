use std::collections::VecDeque;

use bevy_ecs::component::Component;
use mcrs_minecraft_network::Instant;

use crate::world::bus::{OutboundPlayerPacket, PacketPriority};

// Outbound depth thresholds shared by bridge_outbound and dispatch_encode.
// Keeping them here avoids a circular import and ensures both sides enforce
// the same limits.
pub const DEPTH_LIMIT: usize = 256;
pub const DEPTH_DRAIN_TARGET: usize = 192;

// Inbound rate-bucket baselines. A real throughput measurement pass may
// revise these values.
pub const INBOUND_BUCKET_CAP: u32 = 100;
pub const INBOUND_REFILL_PER_SECOND: f32 = 400.0;
pub const INBOUND_KICK_OVERFLOW_PACKETS: u8 = 3;

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
/// Refills from the packets' arrival times rather than from server ticks: a
/// server that stalls drains a backlog in one go, and that burst is the
/// server's, not the client's.
#[derive(Component)]
pub struct InboundRateBucket {
    tokens: f32,
    last_arrival: Option<Instant>,
    overflow: u8,
}

impl Default for InboundRateBucket {
    fn default() -> Self {
        Self::new()
    }
}

impl InboundRateBucket {
    pub fn new() -> Self {
        Self {
            tokens: INBOUND_BUCKET_CAP as f32,
            last_arrival: None,
            overflow: 0,
        }
    }

    /// Spends a token on a packet that arrived at `arrival`, answering `false`
    /// once `INBOUND_KICK_OVERFLOW_PACKETS` packets in a row found the bucket
    /// empty.
    pub fn consume_or_flag(&mut self, arrival: Instant) -> bool {
        if let Some(last) = self.last_arrival {
            let elapsed = arrival.saturating_duration_since(last).as_secs_f32();
            self.tokens =
                (self.tokens + elapsed * INBOUND_REFILL_PER_SECOND).min(INBOUND_BUCKET_CAP as f32);
        }
        self.last_arrival = Some(self.last_arrival.map_or(arrival, |last| last.max(arrival)));
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.overflow = 0;
            true
        } else {
            self.overflow += 1;
            self.overflow < INBOUND_KICK_OVERFLOW_PACKETS
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_backlog_drained_at_once_is_judged_by_when_it_arrived() {
        let mut bucket = InboundRateBucket::new();
        let start = Instant::now();
        for sent in 0..10_000u32 {
            let arrival = start + Duration::from_millis(u64::from(sent) * 5);
            assert!(bucket.consume_or_flag(arrival), "packet {sent} was flagged");
        }
    }

    #[test]
    fn a_burst_past_the_cap_is_flagged() {
        let mut bucket = InboundRateBucket::new();
        let now = Instant::now();
        let accepted = (0..INBOUND_BUCKET_CAP + u32::from(INBOUND_KICK_OVERFLOW_PACKETS))
            .take_while(|_| bucket.consume_or_flag(now))
            .count();
        assert_eq!(
            accepted,
            (INBOUND_BUCKET_CAP + u32::from(INBOUND_KICK_OVERFLOW_PACKETS) - 1) as usize
        );
    }
}
