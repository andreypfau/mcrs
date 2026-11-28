#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod array_kinds;
mod budget;
mod compound_keys;
mod declared_lengths;
mod encodings;
mod equality;
mod merge;
