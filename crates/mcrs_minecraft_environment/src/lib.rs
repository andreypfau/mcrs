#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

pub mod attribute;
#[rustfmt::skip]
pub mod keys;
pub mod spatial;
pub mod timeline;
pub mod world_clock;

mcrs_minecraft_registries::read_with_serde!(
    crate::timeline::Timeline,
    crate::world_clock::WorldClock
);
