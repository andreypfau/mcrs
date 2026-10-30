//! The chat rules that the client and the server share.

#![no_std]

extern crate alloc;

mod max_input;
mod normalize;

pub use max_input::MAX_INPUT;
pub use normalize::normalize;
