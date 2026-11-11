#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
// Every file here is one module of a single test binary; a top-level
// file under tests/ would be a separate crate and a separate link.
mod world_shaders;
