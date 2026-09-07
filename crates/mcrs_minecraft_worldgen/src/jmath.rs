//! Java float semantics. Rust's stdlib disagrees with `java.lang.Math` on signed
//! zero and on division rounding often enough that using the stdlib directly is
//! a silent parity bug.
//!
//! Density values are guaranteed NaN-free, so nothing here propagates NaN. If
//! that guarantee is ever broken these functions return the wrong operand
//! silently; `Program::fill` carries the debug assertion that catches it.

/// `min` and `max` on density values, as the *volume* path computes them: a bare
/// comparison with the left operand as the accumulator. Vanilla's scalar path
/// uses `Math.min` / `Math.max` instead, and the two disagree on signed zero —
/// the parity fixtures are volume dumps, so the volume form is the one to match.
#[inline]
pub fn vmin(left: f32, right: f32) -> f32 {
    if right < left { right } else { left }
}

#[inline]
pub fn vmax(left: f32, right: f32) -> f32 {
    if right > left { right } else { left }
}

/// `Math.min` / `Math.max` for **interval endpoints**, where NaN is
/// `Interval::NAI` — a meaningful "nothing is known" sentinel that must
/// propagate. The no-NaN guarantee covers density values, not bounds.
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

/// `(float)Math.sqrt(x)`. Unlike `log` and `pow` the detour through double is
/// not observable: square root is correctly rounded and double carries more than
/// twice float's significand, so the two roundings collapse into one.
#[inline]
pub fn sqrt(v: f32) -> f32 {
    v.sqrt()
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

/// `Mth.lerp(float delta, float from, float to)`. Always inlined: the noise
/// kernels call it eight times per lattice cell.
///
/// The fast profile fuses the multiply and the add into one rounding, which is
/// the trade `docs/worldgen.md` §15 lists for that profile; the strict profile
/// keeps vanilla's two.
#[inline(always)]
pub fn lerp(delta: f32, from: f32, to: f32) -> f32 {
    #[cfg(feature = "fast")]
    return delta.mul_add(to - from, from);
    #[cfg(not(feature = "fast"))]
    return from + delta * (to - from);
}

/// The lerp every `LerpFunction` sampler computes, which is not `Mth.lerp`: an
/// alpha of exactly zero or one returns that endpoint untouched, where
/// `first + 1 * (second - first)` misses `second` by an ulp.
#[inline]
pub fn sampler_lerp(alpha: f32, first: f32, second: f32) -> f32 {
    if alpha == 0.0 {
        first
    } else if alpha == 1.0 {
        second
    } else {
        lerp(alpha, first, second)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_max_follow_the_volume_path_not_math_min() {
        assert_eq!(vmin(3.0, 5.0), 3.0);
        assert_eq!(vmax(3.0, 5.0), 5.0);
        // Math.min(+0.0, -0.0) is -0.0; the volume path keeps the accumulator.
        assert!(vmin(0.0, -0.0).is_sign_positive());
        assert!(vmax(-0.0, 0.0).is_sign_negative());
    }

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
    fn a_unit_alpha_lerps_to_the_endpoint_exactly() {
        let (first, second) = (-0.524_070_74_f32, 0.088_458_45_f32);
        assert_eq!(sampler_lerp(1.0, first, second), second);
        assert_eq!(sampler_lerp(0.0, first, second), first);
        assert_ne!(
            lerp(1.0, first, second),
            second,
            "the bare Mth.lerp misses the endpoint, which is why the samplers branch"
        );
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
}
