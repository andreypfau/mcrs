#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod array_kinds;
mod declared_lengths;
