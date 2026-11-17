#![doc = include_str!("../README.md")]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_import_braces,
    unreachable_pub,
    clippy::dbg_macro
)]

extern crate self as mcrs_minecraft_wire;

#[doc(hidden)]
pub mod __private {
    pub use crate::{Context, Decode, Encode, Error, ErrorKind, Result, VarInt};
}

mod bounded;
mod error;
mod impls;
mod nested;
mod raw;
mod utf16;
mod var_int;
mod var_long;

use std::io::Write;

pub use bounded::Bounded;
pub use error::{Context, Error, ErrorKind, Result};
pub use mcrs_minecraft_wire_macros::{Decode, Encode};
pub use nested::{MAX_NESTING, nested};
pub use raw::RawBytes;
pub use utf16::{utf16_byte_offset, utf16_len, utf16_position};
pub use var_int::VarInt;
pub use var_long::VarLong;

pub trait Encode {
    fn encode(&self, w: impl Write) -> Result<()>;

    fn encode_slice(slice: &[Self], mut w: impl Write) -> Result<()>
    where
        Self: Sized,
    {
        for value in slice {
            value.encode(&mut w)?;
        }

        Ok(())
    }
}

pub trait Decode<'a>: Sized {
    fn decode(r: &mut &'a [u8]) -> Result<Self>;
}
