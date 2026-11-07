//! The GPU side of Molang: the lowering of a pure channel program to a WGSL function, the WGSL
//! math library that the function calls, and the CPU twin that computes the same bits.

#![no_std]

extern crate alloc;

mod gpu_math;
mod twin;
mod wgsl;

pub use twin::{Twin, TwinRun};
pub use wgsl::{LIBRARY, lower, write_function};
