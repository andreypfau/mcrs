//! Sky-light invariant checks. The implementation currently lives in
//! `crate::invariants`; this module re-exports the sky-side surface
//! as the canonical path.

pub use crate::invariants::{check_sky_light_invariants, SkyInvariantViolation, SkyViolationKind};
