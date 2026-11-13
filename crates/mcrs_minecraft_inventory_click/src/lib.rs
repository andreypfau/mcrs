//! The operations of one Java Edition menu click, for each click kind except the drag. The
//! kernel reads the slots through the caller and writes the request into a caller-owned buffer.
#![no_std]

mod click;
mod click_error;
mod clone;
mod menu;
mod operations;
mod pick_up;
mod pick_up_all;
mod quick_move;
mod request;
mod seen;
mod slots;
mod swap;
mod throw;
mod view;

pub use click::Click;
pub use click::ClickKind;
pub use click::ClickPlayer;
pub use click::Clicked;
pub use click_error::ClickError;
pub use menu::Menu;
pub use operations::Operations;
pub use request::request;
pub use slots::Slots;
