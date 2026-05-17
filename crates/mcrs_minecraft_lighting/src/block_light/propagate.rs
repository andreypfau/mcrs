//! Block-light BFS wrappers and systems. The implementations currently
//! live in `crate::propagate`; this module re-exports the block-side
//! surface so callers can land on `crate::block_light::propagate::*`
//! as the canonical path. A future refactor will move the bodies here.

pub use crate::propagate::{propagate_decrease_block_system, propagate_increase_block_system};
