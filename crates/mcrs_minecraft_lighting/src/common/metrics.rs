//! Process-wide monotone counters for the cross-chunk lighting pipeline.
//!
//! All counters use `Relaxed` ordering because they are summary metrics, not
//! synchronisation primitives. Producers `fetch_add` from any thread; observers
//! call `snapshot()` to read a consistent four-tuple of values.
use std::sync::atomic::{AtomicU64, Ordering};

pub static LIGHT_CONVERGE_ITERATIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static LIGHT_CONVERGE_CAPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static LIGHT_CROSS_DIM_VIOLATIONS_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub struct LightTelemetrySnapshot {
    pub iterations: u64,
    pub capped: u64,
    pub overflow: u64,
    pub cross_dim: u64,
}

pub fn snapshot() -> LightTelemetrySnapshot {
    LightTelemetrySnapshot {
        iterations: LIGHT_CONVERGE_ITERATIONS_TOTAL.load(Ordering::Relaxed),
        capped: LIGHT_CONVERGE_CAPPED_TOTAL.load(Ordering::Relaxed),
        overflow: LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL.load(Ordering::Relaxed),
        cross_dim: LIGHT_CROSS_DIM_VIOLATIONS_TOTAL.load(Ordering::Relaxed),
    }
}

/// Process-wide test serialisation for counter-observing tests. The four
/// telemetry counters are global atomics, so any test that snapshots them
/// before and after a state change must hold this mutex across the
/// observation window — otherwise concurrent tests in the same binary will
/// race and the delta becomes non-deterministic.
///
/// Visible to integration tests in the `tests/` directory: those compile as
/// separate binaries and see the lighting crate as an external dependency
/// without test-config visibility, so the lock cannot be `#[cfg(test)]`-gated.
pub static TELEMETRY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reads_counters() {
        let _lock = TELEMETRY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let before = snapshot();
        LIGHT_CONVERGE_ITERATIONS_TOTAL.fetch_add(7, Ordering::Relaxed);
        LIGHT_CONVERGE_CAPPED_TOTAL.fetch_add(7, Ordering::Relaxed);
        LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL.fetch_add(7, Ordering::Relaxed);
        LIGHT_CROSS_DIM_VIOLATIONS_TOTAL.fetch_add(7, Ordering::Relaxed);
        let after = snapshot();
        assert_eq!(after.iterations - before.iterations, 7);
        assert_eq!(after.capped - before.capped, 7);
        assert_eq!(after.overflow - before.overflow, 7);
        assert_eq!(after.cross_dim - before.cross_dim, 7);
    }
}
