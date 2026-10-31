#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod fixture;

mod light_plugin;
