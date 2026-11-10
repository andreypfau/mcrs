use core::f64::consts::PI;

const RADIANS_PER_DEGREE: f32 = (PI / 180.0) as f32;

/// The angle `degrees` wrapped into `[-180, 180)`. A zero keeps its sign, and a NaN or an
/// infinity gives a NaN.
#[inline]
#[must_use]
pub fn wrap_degrees(degrees: f32) -> f32 {
    // The remainder is a library call, and an angle in the range is its own remainder.
    if (-180.0..180.0).contains(&degrees) {
        return degrees;
    }
    let mut wrapped = degrees % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

/// The angle `degrees` in radians: the f32 product of `degrees` and the f32 value of `π / 180`,
/// widened.
#[inline]
#[must_use]
pub fn radians_from_degrees(degrees: f32) -> f64 {
    f64::from(degrees * RADIANS_PER_DEGREE)
}
