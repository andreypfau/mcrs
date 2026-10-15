#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON"
)]

mod common;

mod asset;
#[cfg(feature = "bevy")]
mod colors;
#[cfg(feature = "bevy")]
mod corpus;
#[cfg(feature = "bevy")]
mod item_light;
mod oracle;
