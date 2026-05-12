#![cfg(any(test, debug_assertions))]

//! Per-cell block-light invariant checker for a single `ChunkSection`.
//!
//! Verifies three invariants for every cell in the 16×16×16 section:
//!
//! 1. **Source floor:** the stored level is at least the cell's own emission.
//! 2. **Support floor:** the stored level is at least the maximum inward
//!    contribution from the six cardinal neighbours, where each neighbour
//!    contributes `neighbour_level - max(1, dampening_of_self)` unless the
//!    cell-cell face is fully occluded.
//! 3. **Source-only excess:** when the stored level exceeds the inward
//!    support, the excess equals the cell's own emission — i.e. only
//!    emitter cells may be brighter than the surrounding field supports.
//!
//! Single-section variant: cells on the section boundary (any of x/y/z is 0
//! or 15) skip the support-floor contribution from the outward-facing
//! neighbour because that neighbour lives in a sibling section the checker
//! cannot resolve from intra-section state alone.
//!
//! Gated under `#[cfg(any(test, debug_assertions))]` because the check is
//! only ever called from tests or from a debug-only verification system —
//! production release builds compile it away entirely.

use mcrs_engine::world::block::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    SourceFloor,
    SupportFloor,
    SourceExcess,
}

#[derive(Debug, Clone, Copy)]
pub struct InvariantViolation {
    pub cell: BlockPos,
    pub stored: u8,
    pub emitted: u8,
    pub max_support: u8,
    pub kind: ViolationKind,
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InvariantViolation {{ kind: {:?}, cell: {:?}, stored: {}, emitted: {}, max_support: {} }}",
            self.kind, self.cell, self.stored, self.emitted, self.max_support
        )
    }
}
