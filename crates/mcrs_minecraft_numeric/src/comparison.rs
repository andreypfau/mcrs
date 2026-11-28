#[must_use]
pub fn min_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

#[must_use]
pub fn max_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}

#[must_use]
pub fn clamp_f32(value: f32, low: f32, high: f32) -> f32 {
    min_f32(high, max_f32(value, low))
}

#[must_use]
pub fn sign_f32(x: f32) -> f32 {
    if x.is_nan() || x == 0.0 {
        x
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}
