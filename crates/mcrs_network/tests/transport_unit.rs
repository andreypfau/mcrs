mod common;

use mcrs_network::metrics::{self, BRIDGE_DROP_NORMAL_TOTAL};
use std::sync::atomic::Ordering;

#[test]
fn metrics_delta_on_drop() {
    let _lock = metrics::TELEMETRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let before = metrics::snapshot();
    BRIDGE_DROP_NORMAL_TOTAL.fetch_add(3, Ordering::Relaxed);
    let after = metrics::snapshot();
    assert_eq!(after.drop_normal_total - before.drop_normal_total, 3);
}
