//! The state id of a menu, the server's view of the stacks that the client holds, and the
//! comparison that finds the entries to send again.
// std: mcrs_minecraft_item_stack

extern crate alloc;

mod agreement;
mod click_sync;
mod client_view;
mod mismatches;
mod slot_view;
mod state_id;

pub use agreement::Agreement;
pub use click_sync::ClickSync;
pub use client_view::ClientView;
pub use mismatches::Mismatches;
pub use slot_view::SlotView;
pub use state_id::StateId;
