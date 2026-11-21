#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod capture;
mod commands;
mod drops;
mod fixture;
mod player_file;
#[cfg(not(miri))]
mod player_item_file;
mod slot_policy;
mod swing;
