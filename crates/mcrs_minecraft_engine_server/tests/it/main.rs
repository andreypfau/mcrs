#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod connection_limits;
mod movement_layers;
mod server;
mod without_vanilla;
