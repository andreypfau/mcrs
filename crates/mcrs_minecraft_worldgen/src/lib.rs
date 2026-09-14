pub mod aquifer;
pub mod beta;
#[cfg(feature = "bevy")]
pub mod bevy;
pub mod bounds;
mod branch;
pub mod carver;
pub mod cell;
pub mod compile;
#[cfg(any(test, feature = "corpus"))]
pub mod corpus;
pub mod feature;
pub mod interval;
pub(crate) mod jmath;
pub(crate) mod kernel;
pub mod material;
pub mod node;
pub mod noise;
pub mod program;
pub mod proto;
pub mod router;
pub mod sample_grid;
pub mod strata;
pub mod structure;
pub mod value_provider;

pub use interval::Interval;
pub use sample_grid::{Axis, SampleGrid};

/// Whether the density and climate samplers were built with the fast precision
/// profile, which downstream parity tests hold to a measured budget instead of
/// bit equality (`docs/worldgen.md` §15).
pub const FAST_PROFILE: bool = cfg!(feature = "fast");
