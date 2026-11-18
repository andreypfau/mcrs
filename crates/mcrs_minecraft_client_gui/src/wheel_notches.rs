use bevy::input::mouse::MouseScrollUnit;

const NOTCHES_PER_LOGICAL_PIXEL: f64 = 0.1;

#[must_use]
pub fn notches_per_unit(unit: MouseScrollUnit, scale_factor: f32) -> f64 {
    match unit {
        MouseScrollUnit::Line => 1.0,
        MouseScrollUnit::Pixel => NOTCHES_PER_LOGICAL_PIXEL / f64::from(scale_factor),
    }
}
