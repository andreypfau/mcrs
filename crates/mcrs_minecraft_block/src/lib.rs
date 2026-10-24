#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
pub mod definition;
#[rustfmt::skip]
pub mod keys;
pub mod material;
