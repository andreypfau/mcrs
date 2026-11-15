//! The state id of a menu and the server's view of the stacks that the client holds.
// std: mcrs_minecraft_item_stack

extern crate alloc;

mod click_sync;
mod client_view;
mod slot_view;
mod state_id;

pub use click_sync::ClickSync;
pub use client_view::ClientView;
pub use slot_view::SlotView;
pub use state_id::StateId;
