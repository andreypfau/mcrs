#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON"
)]

mod environment;
