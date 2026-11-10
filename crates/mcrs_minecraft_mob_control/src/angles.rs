use core::f32::consts::PI;

use mcrs_minecraft_numeric as numeric;

/// `angle` turned toward `goal` by at most `step` degrees.
#[must_use]
pub fn turn_toward(angle: f32, goal: f32, step: f32) -> f32 {
    angle + numeric::wrap_degrees(goal - angle).clamp(-step, step)
}

/// `angle` moved by the smallest amount that puts it within `width` degrees of `centre`.
#[must_use]
pub fn limit_to(angle: f32, centre: f32, width: f32) -> f32 {
    centre - numeric::wrap_degrees(centre - angle).clamp(-width, width)
}

/// The yaw to the offset `(dx, dz)`: `f32(atan2(dz, dx) × 180 / π) - 90`, with the `atan2` and the
/// f32 constants of the reference.
#[must_use]
pub fn yaw_to(dx: f64, dz: f64) -> f32 {
    degrees(numeric::atan2(dz, dx)) - 90.0
}

/// The pitch to an offset with the height `dy` and the horizontal length `horizontal`.
#[must_use]
pub fn pitch_to(dy: f64, horizontal: f64) -> f32 {
    -degrees(numeric::atan2(dy, horizontal))
}

fn degrees(radians: f64) -> f32 {
    (radians * 180.0 / f64::from(PI)) as f32
}
