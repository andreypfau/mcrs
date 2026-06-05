use std::sync::atomic::{AtomicU64, Ordering};

pub static BRIDGE_QUEUE_DEPTH_CRITICAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_QUEUE_DEPTH_HIGH: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_QUEUE_DEPTH_NORMAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_QUEUE_DEPTH_LOW: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_DROP_NORMAL_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_DROP_LOW_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_KICK_OVERFLOW_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_KICK_FLOOD_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_INBOUND_RATE_DROPS: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_HANDSHAKE_INFLIGHT: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_OUTBOUND_MESSAGES_CONSUMED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_ENCODE_UNHANDLED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static BRIDGE_OUTBOUND_NO_QUEUE_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub struct BridgeTelemetrySnapshot {
    pub queue_depth_critical: u64,
    pub queue_depth_high: u64,
    pub queue_depth_normal: u64,
    pub queue_depth_low: u64,
    pub drop_normal_total: u64,
    pub drop_low_total: u64,
    pub kick_overflow_total: u64,
    pub kick_flood_total: u64,
    pub inbound_rate_drops: u64,
    pub handshake_inflight: u64,
    pub outbound_messages_emitted_total: u64,
    pub outbound_messages_consumed_total: u64,
    pub encode_unhandled_total: u64,
    pub outbound_no_queue_total: u64,
}

pub fn snapshot() -> BridgeTelemetrySnapshot {
    BridgeTelemetrySnapshot {
        queue_depth_critical: BRIDGE_QUEUE_DEPTH_CRITICAL.load(Ordering::Relaxed),
        queue_depth_high: BRIDGE_QUEUE_DEPTH_HIGH.load(Ordering::Relaxed),
        queue_depth_normal: BRIDGE_QUEUE_DEPTH_NORMAL.load(Ordering::Relaxed),
        queue_depth_low: BRIDGE_QUEUE_DEPTH_LOW.load(Ordering::Relaxed),
        drop_normal_total: BRIDGE_DROP_NORMAL_TOTAL.load(Ordering::Relaxed),
        drop_low_total: BRIDGE_DROP_LOW_TOTAL.load(Ordering::Relaxed),
        kick_overflow_total: BRIDGE_KICK_OVERFLOW_TOTAL.load(Ordering::Relaxed),
        kick_flood_total: BRIDGE_KICK_FLOOD_TOTAL.load(Ordering::Relaxed),
        inbound_rate_drops: BRIDGE_INBOUND_RATE_DROPS.load(Ordering::Relaxed),
        handshake_inflight: BRIDGE_HANDSHAKE_INFLIGHT.load(Ordering::Relaxed),
        outbound_messages_emitted_total: BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .load(Ordering::Relaxed),
        outbound_messages_consumed_total: BRIDGE_OUTBOUND_MESSAGES_CONSUMED_TOTAL
            .load(Ordering::Relaxed),
        encode_unhandled_total: BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed),
        outbound_no_queue_total: BRIDGE_OUTBOUND_NO_QUEUE_TOTAL.load(Ordering::Relaxed),
    }
}

/// Visible to integration tests in `tests/`: those compile as separate binaries
/// and cannot use `#[cfg(test)]`-gated items from this crate. Tests that
/// snapshot global atomics must hold this lock across the observation window
/// to avoid cross-test counter races in the same process.
pub static TELEMETRY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
