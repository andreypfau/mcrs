/// The natural logarithm of `x`, computed in f64 and narrowed.
#[must_use]
pub fn ln_f32(x: f32) -> f32 {
    f64::from(x).ln() as f32
}

/// `base` to the power `exponent`, computed in f64 and narrowed. Unlike IEEE 754 `pow`, a NaN
/// exponent always gives a NaN, and a base of magnitude `1` with an infinite exponent gives a
/// NaN.
#[must_use]
pub fn pow_f32(base: f32, exponent: f32) -> f32 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) {
        return f32::NAN;
    }
    f64::from(base).powf(f64::from(exponent)) as f32
}
