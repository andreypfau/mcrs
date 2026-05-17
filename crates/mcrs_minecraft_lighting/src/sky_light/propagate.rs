//! Sky-light BFS wrappers and systems, including the column-walker fast
//! path for all-air chunks. The implementations currently live in
//! `crate::propagate`; this module re-exports the sky-side surface so
//! callers can land on `crate::sky_light::propagate::*` as the
//! canonical path. A future refactor will move the bodies here.

pub use crate::propagate::{propagate_decrease_sky_system, propagate_increase_sky_system};
