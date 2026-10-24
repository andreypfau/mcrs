#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
pub mod asset;
pub mod colors;
#[cfg(feature = "bevy")]
pub mod item;
pub mod layout;
#[cfg(feature = "bevy")]
pub mod plugin;
pub mod region;
pub mod resolve;
