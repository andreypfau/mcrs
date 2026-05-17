//! Channel-shared emit-dirty plumbing. `downgrade_light_storage` and
//! `clear_light_tickets` were unified across both channels in earlier
//! refactoring work; they live here as the canonical shared path. The
//! implementations currently sit in `crate::emit_dirty`; this module
//! re-exports the shared surface.

pub use crate::emit_dirty::{clear_light_tickets, downgrade_light_storage};
