//! Java float semantics. Rust's stdlib disagrees with `java.lang.Math` on signed
//! zero and on division rounding often enough that using the stdlib directly is
//! a silent parity bug.

use std::ops::{Add, Div, Mul, Sub};
use std::sync::LazyLock;

pub trait Float:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    const ZERO: Self;
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

/// `Math.min(float, float)`: NaN propagates, and `-0.0` is below `+0.0`.
#[inline]
pub fn jmin(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

/// `Math.max(float, float)`: NaN propagates, and `+0.0` is above `-0.0`.
#[inline]
pub fn jmax(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// `Math.signum(float)`. Zero keeps its sign, which Rust's `f32::signum` does
/// not do — it returns `±1.0` for both zeros.
#[inline]
pub fn signum(v: f32) -> f32 {
    if v == 0.0 { v } else { 1.0_f32.copysign(v) }
}

/// `(float)Math.log(x)`. Java has no float overload, so the value widens to
/// `double`, the log is taken at double precision, and the result narrows.
/// Computing `f32::ln` directly rounds once instead of twice and drifts.
#[inline]
pub fn log(v: f32) -> f32 {
    (v as f64).ln() as f32
}

/// `(float)Math.pow(a, b)`, likewise computed at double precision.
#[inline]
pub fn pow(a: f32, b: f32) -> f32 {
    (a as f64).powf(b as f64) as f32
}

/// `Mth.clamp(float, float, float)`. Returns `min` when the value is NaN,
/// because `NaN < min` is false and `NaN > max` is false, leaving the value —
/// vanilla's `Mth.clamp` is written as nested ternaries with the same outcome.
#[inline]
pub fn clampf(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// `Mth.floor(float)`, which is `(int)Math.floor(v)`: the narrowing cast
/// saturates at the integer bounds and sends NaN to zero rather than wrapping.
#[inline]
pub fn mth_floor(v: f32) -> i32 {
    (v as f64).floor() as i32
}

/// `Math.floorDiv`. Rust's `/` truncates toward zero, and `div_euclid` rounds
/// toward negative infinity only for a positive divisor: `floorDiv(-1, -4)` is
/// 0 where `(-1).div_euclid(-4)` is 1.
#[inline]
pub fn floor_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    if (a ^ b) < 0 && q * b != a { q - 1 } else { q }
}

/// `Math.floorMod`. Takes the sign of the divisor, where Rust's `%` takes the
/// sign of the dividend and `rem_euclid` is always non-negative.
#[inline]
pub fn floor_mod(a: i32, b: i32) -> i32 {
    a - floor_div(a, b) * b
}

/// `Mth.fastInvSqrt`: one Newton step from a bit-trick guess, so it is not
/// `1 / sqrt(x)` and the beard term needs this exact approximation.
#[inline]
pub fn fast_inv_sqrt(x: f64) -> f64 {
    let half = 0.5 * x;
    let guess =
        f64::from_bits(6910469410427058090_i64.wrapping_sub((x.to_bits() as i64) >> 1) as u64);
    guess * (1.5 - half * guess * guess)
}

/// `Mth.clampedMap`: a lerp over an inverse lerp with the source range as a hard
/// clamp, so a value outside it maps to the near end of the target range.
pub fn clamped_map<F: Float>(value: F, from_min: F, from_max: F, to_min: F, to_max: F) -> F {
    let delta = (value - from_min) / (from_max - from_min);
    if delta < F::ZERO {
        to_min
    } else if delta > F::ONE {
        to_max
    } else {
        to_min + delta * (to_max - to_min)
    }
}

/// `Mth.lerp`.
pub fn lerp<F: Float>(alpha: F, from: F, to: F) -> F {
    from + alpha * (to - from)
}

/// `Mth.lerpInt`: the step is floored, so the result leaves `from` only once a
/// whole unit has accumulated.
pub fn lerp_int(alpha: f32, from: i32, to: i32) -> i32 {
    from.wrapping_add((alpha * to.wrapping_sub(from) as f32).floor() as i32)
}

/// `Mth.wrapDegrees(float)`, which leaves an already-in-range angle bit-identical.
pub fn wrap_degrees(angle: f32) -> f32 {
    let wrapped = angle % 360.0;
    if wrapped >= 180.0 {
        wrapped - 360.0
    } else if wrapped < -180.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

/// The 65536-entry sine table both Beta's `MathHelper` and 26.3's `Mth` read:
/// `(float)Math.sin(i * 2π / 65536)`.
///
/// The reference quantizes the argument to a table index, so reading the table is
/// not an approximation of that: it is the same value, without paying a `sin` per
/// call.
static SIN_TABLE: LazyLock<Box<[f32; 65536]>> = LazyLock::new(|| {
    let mut table = vec![0.0f32; 65536].into_boxed_slice();
    for (index, entry) in table.iter_mut().enumerate() {
        *entry = f64::sin(index as f64 * (std::f64::consts::TAU / 65536.0)) as f32;
    }
    table.try_into().expect("65536 entries")
});

/// Java beta `MathHelper.sin(x)`: `SIN_TABLE[(int)(x * 10430.378F) & 65535]`.
#[inline]
pub fn sin(x: f32) -> f32 {
    SIN_TABLE[(((x * 10430.378_f32) as i32 as u32) & 0xFFFF) as usize]
}

/// Java beta `MathHelper.cos(x)`: the same table, offset by a quarter turn.
#[inline]
pub fn cos(x: f32) -> f32 {
    SIN_TABLE[(((x * 10430.378_f32 + 16384.0_f32) as i32 as u32) & 0xFFFF) as usize]
}

/// Java 26.3 `Mth.sin(double)`: `SIN[(int)((long)(i * 10430.378350470453) & 65535L)]`.
/// The argument and the scale are `double` here, so the index differs from the Beta
/// form on arguments whose `f32` product rounds across an integer boundary.
#[inline]
pub fn sin_modern(x: f64) -> f32 {
    SIN_TABLE[(((x * 10430.378350470453) as i64 as u64) & 0xFFFF) as usize]
}

/// Java 26.3 `Mth.cos(double)`: the same table, offset by a quarter turn.
#[inline]
pub fn cos_modern(x: f64) -> f32 {
    SIN_TABLE[(((x * 10430.378350470453 + 16384.0) as i64 as u64) & 0xFFFF) as usize]
}

/// A lerp over an inverse lerp, unclamped: a value outside the source range
/// maps outside the target range.
pub fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (value - from_min) / (from_max - from_min) * (to_max - to_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signum_keeps_zero_sign() {
        assert!(signum(0.0).is_sign_positive() && signum(0.0) == 0.0);
        assert!(signum(-0.0).is_sign_negative() && signum(-0.0) == 0.0);
        assert_eq!(signum(-3.0), -1.0);
        assert_eq!(signum(3.0), 1.0);
        assert_eq!((-0.0f32).signum(), -1.0, "stdlib differs, as documented");
    }

    #[test]
    fn log_rounds_once_through_f64() {
        // 0.1f32 is the classic case where ln at f32 precision and ln at f64
        // precision narrowed to f32 disagree in the last bit.
        let v = 0.1_f32;
        assert_eq!(log(v), (v as f64).ln() as f32);
    }

    #[test]
    fn mth_floor_saturates_instead_of_wrapping() {
        assert_eq!(mth_floor(-7.5), -8);
        assert_eq!(mth_floor(7.5), 7);
        assert_eq!(mth_floor(1e30), i32::MAX);
        assert_eq!(mth_floor(-1e30), i32::MIN);
        assert_eq!(mth_floor(f32::NAN), 0);
    }

    #[test]
    fn floor_div_matches_java() {
        assert_eq!(floor_div(-7, 4), -2);
        assert_eq!(floor_mod(-7, 4), 1);
        assert_eq!(-7 / 4, -1, "stdlib truncates, as documented");
    }

    #[test]
    fn floor_div_matches_java_for_a_negative_divisor() {
        assert_eq!(floor_div(-1, -4), 0);
        assert_eq!(floor_mod(-1, -4), -1);
        assert_eq!(floor_div(-7, -4), 1);
        assert_eq!(floor_mod(-7, -4), -3);
        assert_eq!(floor_div(7, -4), -2);
        assert_eq!(floor_mod(7, -4), -1);
        assert_eq!((-1i32).div_euclid(-4), 1, "euclid differs, as documented");
    }

    #[test]
    fn fast_inv_sqrt_approximates_the_inverse_square_root() {
        for x in [0.25, 1.0, 2.0, 50.5, 1e6] {
            let exact = 1.0 / f64::sqrt(x);
            assert!((fast_inv_sqrt(x) - exact).abs() / exact < 0.01, "{x}");
        }
    }

    /// The table has to answer exactly what computing the entry on the fly did.
    #[test]
    fn the_table_matches_the_computed_entry() {
        for step in 0..10_000 {
            let x = (step as f32 - 5_000.0) * 0.0037;
            let sin_index = ((x * 10430.378_f32) as i32 as u32) & 0xFFFF;
            let cos_index = ((x * 10430.378_f32 + 16384.0_f32) as i32 as u32) & 0xFFFF;
            let entry = |i: u32| f64::sin(i as f64 * (std::f64::consts::TAU / 65536.0)) as f32;
            assert_eq!(sin(x), entry(sin_index), "sin({x})");
            assert_eq!(cos(x), entry(cos_index), "cos({x})");
        }
    }

    #[test]
    fn the_modern_index_is_the_double_one() {
        let entry = |i: u32| f64::sin(i as f64 * (std::f64::consts::TAU / 65536.0)) as f32;
        for step in 0..10_000 {
            let x = (step as f64 - 5_000.0) * 0.0037;
            let sin_index = (((x * 10430.378350470453) as i64 as u64) & 0xFFFF) as u32;
            let cos_index = (((x * 10430.378350470453 + 16384.0) as i64 as u64) & 0xFFFF) as u32;
            assert_eq!(sin_modern(x), entry(sin_index), "sin_modern({x})");
            assert_eq!(cos_modern(x), entry(cos_index), "cos_modern({x})");
        }
    }

    /// The two index forms are not interchangeable, which is why both exist.
    #[test]
    fn the_beta_index_and_the_modern_index_disagree() {
        let mut disagreements = 0;
        for step in 0..100_000 {
            let x = (step as f64 - 50_000.0) * 0.0037;
            if sin_modern(x) != sin(x as f32) {
                disagreements += 1;
            }
        }
        assert!(
            disagreements > 0,
            "the two sine indices must not coincide everywhere"
        );
    }
}
