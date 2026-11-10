#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod font;
