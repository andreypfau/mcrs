//! Block-light emit-dirty system and safety-net sweep. The
//! implementations currently live in `crate::emit_dirty`; this module
//! re-exports the block-side surface so callers can land on
//! `crate::block_light::emit_dirty::*` as the canonical path.

pub use crate::emit_dirty::{clear_block_bfs_pending_safety_net, emit_block_light_dirty};
