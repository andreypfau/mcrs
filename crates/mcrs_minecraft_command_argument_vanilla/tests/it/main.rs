#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod feedback;
mod tree;

use mcrs_minecraft_command::{CommandLevel, NoLookup, Source};

pub const OP: Source<'static, NoLookup> = Source::new(CommandLevel::Gamemasters, &NoLookup);
pub const GUEST: Source<'static, NoLookup> = Source::new(CommandLevel::All, &NoLookup);
