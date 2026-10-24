#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod spawn_condition;
mod structure_placement_parity;
