#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]

mod clone;
mod fixture;
mod pick_up;
mod pick_up_all;
mod quick_move;
mod swap;
mod throw;
