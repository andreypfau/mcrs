#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod diodes;
mod observer;
mod piston;
mod plates;
mod power;
mod sources;
mod wire;
mod world;
