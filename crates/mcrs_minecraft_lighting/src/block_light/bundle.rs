//! Block-light bundle. The implementation currently lives in
//! `crate::bundle`; this module re-exports the block-side surface so
//! callers can land on `crate::block_light::bundle::BlockLightBundle`
//! as the canonical path. A future refactor will move the body here.

pub use crate::bundle::BlockLightBundle;
