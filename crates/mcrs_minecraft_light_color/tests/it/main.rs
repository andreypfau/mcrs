#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
#![expect(clippy::panic, reason = "a test reports a failure by panicking")]
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON"
)]

mod common;

mod asset;
mod colors;
mod corpus;
mod item_light;
mod oracle;
