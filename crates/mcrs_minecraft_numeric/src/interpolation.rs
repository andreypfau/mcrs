use crate::conversion::floor_to_i32;
use crate::float::Float;

/// `start + factor * (end - start)`, in that step order. A factor outside `[0, 1]`
/// extrapolates.
#[must_use]
pub fn lerp<F: Float>(factor: F, start: F, end: F) -> F {
    start + factor * (end - start)
}

/// The integer interpolation: `start` plus the floor of `factor * (end - start)`, with wrapping
/// integer steps.
#[must_use]
pub fn lerp_i32(factor: f32, start: i32, end: i32) -> i32 {
    let product = factor * end.wrapping_sub(start) as f32;
    start.wrapping_add(floor_to_i32(product))
}

/// `value` mapped from `[from_start, from_end]` onto `[to_start, to_end]`, with the progress
/// clamped to `[0, 1]`. A NaN progress gives a NaN.
#[must_use]
pub fn remap_clamped<F: Float>(value: F, from_start: F, from_end: F, to_start: F, to_end: F) -> F {
    let progress = (value - from_start) / (from_end - from_start);
    if progress < F::ZERO {
        to_start
    } else if progress > F::ONE {
        to_end
    } else {
        lerp(progress, to_start, to_end)
    }
}

/// `value` mapped from `[from_start, from_end]` onto `[to_start, to_end]`, without a clamp.
#[must_use]
pub fn remap(value: f64, from_start: f64, from_end: f64, to_start: f64, to_end: f64) -> f64 {
    to_start + ((value - from_start) / (from_end - from_start)) * (to_end - to_start)
}
