#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod cell_bounds;
