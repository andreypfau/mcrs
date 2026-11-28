use core::f64::consts::PI;

const RADIANS_PER_DEGREE: f32 = (PI / 180.0) as f32;

#[inline]
#[must_use]
pub fn wrap_degrees(degrees: f32) -> f32 {
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

#[inline]
#[must_use]
pub fn wrap_degrees_f64(degrees: f64) -> f64 {
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

#[inline]
#[must_use]
pub fn radians_from_degrees(degrees: f32) -> f64 {
    f64::from(degrees * RADIANS_PER_DEGREE)
}
