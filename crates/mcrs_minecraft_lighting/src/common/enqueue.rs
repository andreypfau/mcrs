//! Channel-shared enqueue helpers. `consume_needs_full_reseed` operates on
//! both channels and is registered once per tick. The implementation
//! currently lives in `crate::enqueue`; this module re-exports the shared
//! surface as the canonical path.

pub use crate::enqueue::consume_needs_full_reseed;
