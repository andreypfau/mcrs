#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod connection_limits;
mod movement_layers;
mod new_world;
mod server;
mod server_thread;
mod without_vanilla;
