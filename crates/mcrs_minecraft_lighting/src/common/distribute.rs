//! Cross-chunk distribute pass. The implementation operates channel-
//! generically via the `DrainChannel` trait and currently lives in
//! `crate::distribute`; this module re-exports the shared surface as
//! the canonical path.

pub use crate::distribute::distribute_cross_chunk_wavefronts;
