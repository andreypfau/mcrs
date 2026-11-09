use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch; 0 when the clock reads before it.
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
