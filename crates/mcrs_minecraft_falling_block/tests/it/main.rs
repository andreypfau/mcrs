#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a test fails by panicking"
)]

mod concrete_pairs;
mod fixture;
mod landing;
mod start;
mod tick;
mod trigger;
