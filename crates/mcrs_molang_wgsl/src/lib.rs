//! The GPU side of Molang: the lowering of a pure channel program to a WGSL function and the WGSL
//! math library that the function calls.

#![no_std]

extern crate alloc;

mod wgsl;

pub use wgsl::{LIBRARY, lower, write_function};
