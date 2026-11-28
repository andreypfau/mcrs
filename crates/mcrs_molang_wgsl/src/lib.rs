#![no_std]

extern crate alloc;

mod wgsl;

pub use wgsl::{LIBRARY, lower, write_function};
