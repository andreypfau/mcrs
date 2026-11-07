#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod artefact;
mod convert;
mod geometry_rules;
mod mob_assets;
mod parity;
