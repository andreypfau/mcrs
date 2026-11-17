#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod gpu;
mod terrain_pipeline;
mod terrain_seams;
mod world_shaders;
