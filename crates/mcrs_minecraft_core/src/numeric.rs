//! Numeric operations whose result bits go into terrain, command output and registry data.
//!
//! Each function gives a bit-exact result. Most of them differ from the obvious standard library
//! call in a NaN, a signed zero or a step order, so a caller that needs the reference bits uses
//! them.

use core::f64::consts::{FRAC_PI_2, PI};
use core::mem;
use core::ops::{Add, Div, Mul, Sub};

use crate::atan_table::{ARCSINE_BITS, COSINE_OF_ARCSINE_BITS};
use crate::sine_table::{ENTRIES_PER_RADIAN, current_entry, earlier_entry};

pub use crate::sine_table::{SINE_TABLE_LEN, earlier_sine_table, sine_table};

const QUARTER_TURN_ENTRIES: f64 = 16384.0;
const BETA_ENTRIES_PER_RADIAN: f32 = f32::from_bits(0x4622_f983);

/// An IEEE 754 float in which [`lerp`] and [`remap_clamped`] compute.
pub trait Float:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    /// The value `0`.
    const ZERO: Self;
    /// The value `1`.
    const ONE: Self;
}

impl Float for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
}

impl Float for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
}

/// The largest integer not more than `x`. A NaN gives `0`, and a value out of range saturates.
#[must_use]
pub fn floor_to_i32(x: f32) -> i32 {
    x.floor() as i32
}

/// The smallest integer not less than `x`. A NaN gives `0`, and a value out of range saturates.
#[must_use]
pub fn ceil_to_i32(x: f32) -> i32 {
    x.ceil() as i32
}

/// `x` truncated toward zero. A NaN gives `0`, and a value out of range saturates.
#[must_use]
pub fn truncate_to_i32(x: f32) -> i32 {
    x as i32
}

/// The quotient of `a / b`, rounded toward negative infinity. `i32::MIN / -1` wraps to
/// `i32::MIN`. A zero `b` is outside the domain and gives `0`.
#[must_use]
pub fn floor_div(a: i32, b: i32) -> i32 {
    if b == 0 {
        return 0;
    }
    let quotient = a.wrapping_div(b);
    if a.wrapping_rem(b) != 0 && (a ^ b) < 0 {
        quotient.wrapping_sub(1)
    } else {
        quotient
    }
}

/// The remainder of [`floor_div`]: `0` or a value with the sign of `b`. A zero `b` is outside
/// the domain and gives `a`.
#[must_use]
pub fn floor_mod(a: i32, b: i32) -> i32 {
    a.wrapping_sub(floor_div(a, b).wrapping_mul(b))
}

/// The lesser of `a` and `b`, where `-0` is less than `+0`. A NaN operand gives a NaN.
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

/// The greater of `a` and `b`, where `+0` is greater than `-0`. A NaN operand gives a NaN.
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

/// `value` limited to `[low, high]` by [`max_f32`] and then [`min_f32`]. A NaN `value` gives a
/// NaN. The bounds must not be NaN, and `low` must not be more than `high`.
#[must_use]
pub fn clamp_f32(value: f32, low: f32, high: f32) -> f32 {
    min_f32(high, max_f32(value, low))
}

/// The sign of `x`: `1` or `-1`. A zero and a NaN give themselves.
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

/// `start + factor * (end - start)`, in that step order. A factor outside `[0, 1]`
/// extrapolates.
#[must_use]
pub fn lerp<F: Float>(factor: F, start: F, end: F) -> F {
    start + factor * (end - start)
}

/// The integer interpolation: `start` plus the floor of `factor * (end - start)`, with wrapping
/// integer steps.
#[must_use]
pub fn lerp_i32(factor: f32, start: i32, end: i32) -> i32 {
    let product = factor * end.wrapping_sub(start) as f32;
    start.wrapping_add(floor_to_i32(product))
}

/// `value` mapped from `[from_start, from_end]` onto `[to_start, to_end]`, with the progress
/// clamped to `[0, 1]`. A NaN progress gives a NaN.
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

/// `value` mapped from `[from_start, from_end]` onto `[to_start, to_end]`, without a clamp.
#[must_use]
pub fn remap(value: f64, from_start: f64, from_end: f64, to_start: f64, to_end: f64) -> f64 {
    to_start + ((value - from_start) / (from_end - from_start)) * (to_end - to_start)
}

/// The angle `degrees` wrapped into `[-180, 180)`. A zero keeps its sign, and a NaN or an
/// infinity gives a NaN.
#[must_use]
pub fn wrap_degrees(degrees: f32) -> f32 {
    let mut wrapped = degrees % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

/// The sine of `radians` from the current table, at the nearest entry. The result is odd in
/// `radians`, and a NaN gives `-0`.
#[must_use]
pub fn sin_nearest(radians: f64) -> f32 {
    if radians >= 0.0 {
        current_entry(nearest_index(radians, 0.0))
    } else {
        -current_entry(nearest_index(-radians, 0.0))
    }
}

/// The cosine of `radians` from the current table, at the nearest entry. A NaN gives `0`.
#[must_use]
pub fn cos_nearest(radians: f64) -> f32 {
    let magnitude = if radians >= 0.0 { radians } else { -radians };
    current_entry(nearest_index(magnitude, QUARTER_TURN_ENTRIES))
}

fn nearest_index(radians: f64, offset: f64) -> u16 {
    (radians * ENTRIES_PER_RADIAN + offset + 0.5) as i64 as u16
}

/// The sine of `radians` from the earlier table. The index truncates toward zero, so a negative
/// angle takes the entry below the nearest one.
#[must_use]
pub fn sin_truncated(radians: f64) -> f32 {
    earlier_entry((radians * ENTRIES_PER_RADIAN) as i64 as u16)
}

/// The cosine of `radians` from the earlier table, with the index of [`sin_truncated`].
#[must_use]
pub fn cos_truncated(radians: f64) -> f32 {
    earlier_entry((radians * ENTRIES_PER_RADIAN + QUARTER_TURN_ENTRIES) as i64 as u16)
}

/// The sine of `radians` from the earlier table, with the f32 index of the Beta releases.
#[must_use]
pub fn sin_beta(radians: f32) -> f32 {
    earlier_entry((radians * BETA_ENTRIES_PER_RADIAN) as i32 as u16)
}

/// The cosine of `radians` from the earlier table, with the f32 index of the Beta releases.
#[must_use]
pub fn cos_beta(radians: f32) -> f32 {
    earlier_entry((radians * BETA_ENTRIES_PER_RADIAN + 16384.0) as i32 as u16)
}

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

/// The packed `0xRRGGBB` colour of a hue, a saturation and a value. A hue of `-1/6` or less is
/// outside the domain and gives `0`.
#[must_use]
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> i32 {
    let sextant_position = hue * 6.0;
    let sextant = truncate_to_i32(sextant_position) % 6;
    let fraction = sextant_position - sextant as f32;
    let low = value * (1.0 - saturation);
    let falling = value * (1.0 - fraction * saturation);
    let rising = value * (1.0 - (1.0 - fraction) * saturation);
    let [red, green, blue] = match sextant {
        0 => [value, rising, low],
        1 => [falling, value, low],
        2 => [low, value, rising],
        3 => [low, falling, value],
        4 => [rising, low, value],
        5 => [value, low, falling],
        _ => return 0,
    };
    channel(red) << 16 | channel(green) << 8 | channel(blue)
}

fn channel(component: f32) -> i32 {
    truncate_to_i32(component * 255.0).clamp(0, 255)
}
