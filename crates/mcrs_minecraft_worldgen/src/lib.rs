#![expect(
    clippy::expect_used,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
pub mod beard;
#[cfg(feature = "bevy")]
pub mod bevy;
#[cfg(feature = "bevy")]
pub mod plugin;
pub mod registries;
pub mod router;
#[cfg(feature = "bevy")]
pub mod tables;
