#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod conformance_chain;
mod conformance_set;
mod conformance_tick;
mod determinism;
mod events;
mod probe;
mod sequence;
mod tick_model;
mod ticks;
