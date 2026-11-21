#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod diodes;
mod mechanisms;
mod observer;
mod piston;
mod plates;
mod power;
mod sources;
mod vanilla_server;
mod wire;
mod world;
