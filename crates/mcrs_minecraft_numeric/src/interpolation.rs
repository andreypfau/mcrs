use crate::conversion::floor_to_i32;
use crate::float::Float;

#[must_use]
pub fn lerp<F: Float>(factor: F, start: F, end: F) -> F {
    start + factor * (end - start)
}

#[must_use]
pub fn lerp_i32(factor: f32, start: i32, end: i32) -> i32 {
    let product = factor * end.wrapping_sub(start) as f32;
    start.wrapping_add(floor_to_i32(product))
}

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

#[must_use]
pub fn remap(value: f64, from_start: f64, from_end: f64, to_start: f64, to_end: f64) -> f64 {
    to_start + ((value - from_start) / (from_end - from_start)) * (to_end - to_start)
}
