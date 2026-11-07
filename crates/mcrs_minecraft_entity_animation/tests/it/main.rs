#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod artefact;
mod batch;
mod mob_assets;
mod pack;
mod parity;
mod rules;
