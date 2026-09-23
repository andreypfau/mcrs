pub mod aquifer;
pub mod beta;
pub mod bounds;
mod branch;
pub mod cell;
pub mod compile;
pub mod node;
pub mod program;
pub mod proto;
pub mod router;

/// Whether the density and climate samplers were built with the fast precision
/// profile, which downstream parity tests hold to a measured budget instead of
/// bit equality (`docs/worldgen.md` §15).
pub const FAST_PROFILE: bool = cfg!(feature = "fast");
