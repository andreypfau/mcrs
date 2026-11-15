#![expect(
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod atomicity;
mod holder;
mod operations;
mod rejections;
