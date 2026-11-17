#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod boot;
#[cfg(feature = "singleplayer")]
mod local_server;
mod session;
mod sky;
#[cfg(feature = "singleplayer")]
mod world_list;
