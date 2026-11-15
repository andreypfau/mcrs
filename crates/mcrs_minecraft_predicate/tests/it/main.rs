#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod allocation;
mod distance;
mod grid;
mod item_predicate;
mod location;
mod state_order;
