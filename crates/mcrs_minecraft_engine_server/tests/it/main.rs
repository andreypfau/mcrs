#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod movement_layers;
mod server;
mod without_vanilla;
