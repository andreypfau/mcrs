#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod entries;
mod network;
mod references;
mod refs;
mod reload;
mod report;
mod static_key;
mod support;
mod tags;
