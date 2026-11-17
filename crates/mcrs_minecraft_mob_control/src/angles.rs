use core::f32::consts::PI;

use mcrs_minecraft_numeric as numeric;

#[must_use]
pub fn turn_toward(angle: f32, goal: f32, step: f32) -> f32 {
    angle + numeric::wrap_degrees(goal - angle).clamp(-step, step)
}

#[must_use]
pub fn limit_to(angle: f32, centre: f32, width: f32) -> f32 {
    centre - numeric::wrap_degrees(centre - angle).clamp(-width, width)
}

#[must_use]
pub fn yaw_to(dx: f64, dz: f64) -> f32 {
    degrees(numeric::atan2(dz, dx)) - 90.0
}

#[must_use]
pub fn pitch_to(dy: f64, horizontal: f64) -> f32 {
    -degrees(numeric::atan2(dy, horizontal))
}

fn degrees(radians: f64) -> f32 {
    (radians * 180.0 / f64::from(PI)) as f32
}
