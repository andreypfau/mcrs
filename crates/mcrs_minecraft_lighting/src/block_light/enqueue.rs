//! Block-light enqueue systems. The implementations currently live in
//! `crate::enqueue`; this module re-exports the block-side surface so
//! callers can land on `crate::block_light::enqueue::*` as the
//! canonical path. A future refactor will move the bodies here.

pub use crate::enqueue::{
    enqueue_block_light_on_block_placed, pull_block_neighbor_edges, seed_block_emitters,
};
