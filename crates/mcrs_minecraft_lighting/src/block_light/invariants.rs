//! Block-light invariant checks. The implementation currently lives in
//! `crate::invariants`; this module re-exports the block-side surface
//! as the canonical path.

pub use crate::invariants::{check_block_light_invariants, InvariantViolation, ViolationKind};
