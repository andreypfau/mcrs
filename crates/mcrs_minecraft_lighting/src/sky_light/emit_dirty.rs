//! Sky-light emit-dirty system and safety-net sweep. The
//! implementations currently live in `crate::emit_dirty`; this module
//! re-exports the sky-side surface so callers can land on
//! `crate::sky_light::emit_dirty::*` as the canonical path.

pub use crate::emit_dirty::{clear_sky_bfs_pending_safety_net, emit_sky_light_dirty};
