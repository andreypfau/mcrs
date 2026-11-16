#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod determinism;
mod pig_goals;
