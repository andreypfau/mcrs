//! Implementations of [`Encode`](crate::Encode) and [`Decode`](crate::Decode)
//! on foreign types.

mod map;
mod math;
mod other;
mod pointer;
mod primitive;
mod sequence;
mod string;
mod tuple;

/// Prevents preallocating too much memory in case we get a malicious or invalid
/// sequence length.
const MAX_PREALLOC_BYTES: usize = 1024 * 1024;
