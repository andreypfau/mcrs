// Proving the wgpu device `Sync` for a static overflows the default limit.
#![recursion_limit = "256"]

pub mod candidates;
#[cfg(feature = "corpus")]
pub mod corpus;
pub mod fixture;
pub mod shade;
