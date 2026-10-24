#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod common;

mod batching;
mod incremental_terrain;
#[cfg(feature = "bevy")]
mod plugin;
mod seams;
mod semantics;
mod sky_surface_bound;
