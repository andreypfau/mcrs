use core::f64::consts::{FRAC_PI_2, PI};
use core::mem;

use crate::atan_table::{ARCSINE_BITS, COSINE_OF_ARCSINE_BITS};

/// An estimate of `1 / √x` for `x > 0`, with a relative error below 0.2 %. It is not
/// `1 / x.sqrt()`.
#[must_use]
pub fn inverse_sqrt_estimate(x: f64) -> f64 {
    let bits = x.to_bits() as i64;
    let guess = f64::from_bits(0x5fe6_eb50_c7b5_37aa_i64.wrapping_sub(bits >> 1) as u64);
    let half = 0.5 * x;
    guess * (1.5 - half * guess * guess)
}

/// The angle of the point `(x, y)` from the positive x axis, in radians. It is within `1e-5` of
/// the exact arctangent when `x² + y²` is a normal number, and it is not the arctangent of a
/// standard library.
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    const ROUNDING_OFFSET: f64 = 17_592_186_044_416.0;

    let length_sq = x * x + y * y;
    if length_sq.is_nan() {
        return f64::NAN;
    }
    let below_x_axis = y < 0.0;
    let mut y = if below_x_axis { -y } else { y };
    let left_of_y_axis = x < 0.0;
    let mut x = if left_of_y_axis { -x } else { x };
    let steep = y > x;
    if steep {
        mem::swap(&mut x, &mut y);
    }
    let inverse_length = inverse_sqrt_estimate(length_sq);
    x *= inverse_length;
    y *= inverse_length;
    let rounded = ROUNDING_OFFSET + y;
    let row = (rounded.to_bits() & 0xffff_ffff) as usize;
    let snapped = rounded - ROUNDING_OFFSET;
    let (Some(&arcsine), Some(&cosine)) = (ARCSINE_BITS.get(row), COSINE_OF_ARCSINE_BITS.get(row))
    else {
        return f64::NAN;
    };
    let error = y * f64::from_bits(cosine) - x * snapped;
    let mut angle = f64::from_bits(arcsine) + ((6.0 + error * error) * error) * 0.16666666666666666;
    if steep {
        angle = FRAC_PI_2 - angle;
    }
    if left_of_y_axis {
        angle = PI - angle;
    }
    if below_x_axis {
        angle = -angle;
    }
    angle
}
