#[must_use]
pub fn ln_f32(x: f32) -> f32 {
    f64::from(x).ln() as f32
}

#[must_use]
pub fn pow_f32(base: f32, exponent: f32) -> f32 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) {
        return f32::NAN;
    }
    f64::from(base).powf(f64::from(exponent)) as f32
}
