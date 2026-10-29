#![no_std]

mod span;
mod utf16;
mod whitespace;

pub use span::Span;
pub use utf16::{byte_offset, utf16_len, utf16_position, utf16_prefix};
pub use whitespace::is_whitespace;
