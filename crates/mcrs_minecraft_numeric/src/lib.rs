//! Numeric operations whose result bits go into terrain, entity motion, command output and
//! registry data.
//!
//! Each function gives a bit-exact result. Most of them differ from the obvious standard library
//! call in a NaN, a signed zero or a step order, so a caller that needs the reference bits uses
//! them.

// std: LazyLock for the sine tables, and the f64 sine, logarithm and power that build and use them

mod angle;
mod atan;
mod atan_table;
mod colour;
mod comparison;
mod conversion;
mod exponential;
mod float;
mod interpolation;
mod sine;
mod sine_table;

pub use angle::{radians_from_degrees, wrap_degrees};
pub use atan::{atan2, inverse_sqrt_estimate};
pub use colour::hsv_to_rgb;
pub use comparison::{clamp_f32, max_f32, min_f32, sign_f32};
pub use conversion::{ceil_to_i32, floor_div, floor_mod, floor_to_i32, truncate_to_i32};
pub use exponential::{ln_f32, pow_f32};
pub use float::Float;
pub use interpolation::{lerp, lerp_i32, remap, remap_clamped};
pub use sine::{cos_beta, cos_nearest, cos_truncated, sin_beta, sin_nearest, sin_truncated};
pub use sine_table::{SINE_TABLE_LEN, earlier_sine_table, sine_table};
