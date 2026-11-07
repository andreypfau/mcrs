//! The GPU math of `library.wgsl`, operation for operation, on the CPU. Each function here and
//! its WGSL namesake must change together: the GPU fuzz test compares their bits.
//!
//! Every float result goes through [`flush`], as on the GPU. Division, square root and remainder
//! use integer long division, so their bits are the correctly rounded results.

use mcrs_molang::{BinaryFn, EaseDirection, EaseShape, Easing, TernaryFn, UnaryFn};

const SIGN: u32 = 0x8000_0000;
const EXPONENT: u32 = 0x7f80_0000;
const MANTISSA: u32 = 0x007f_ffff;
const IMPLICIT: u32 = 0x0080_0000;
const QUIET_NAN: u32 = 0x7fc0_0000;

const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
const RADIANS_TO_DEGREES: f32 = f32::from_bits(0x4265_2ee0);
const ONE_OVER_90: f32 = f32::from_bits(0x3c36_0b61);
const LOG2_E: f32 = f32::from_bits(0x3fb8_aa3b);
const LN_2: f32 = f32::from_bits(0x3f31_7218);
const LN_2_HIGH: f32 = f32::from_bits(0x3f31_7200);
const LN_2_LOW: f32 = f32::from_bits(0x35bf_be8e);
const SQRT_2: f32 = f32::from_bits(0x3fb5_04f3);
const PI: f32 = f32::from_bits(0x4049_0fdb);
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const QUARTER_PI: f32 = f32::from_bits(0x3f49_0fdb);
const TAN_EIGHTH_PI: f32 = f32::from_bits(0x3ed4_13cd);
const APPROX_EQ_TOLERANCE: f32 = 0.000_000_1;
const POSE_LIMIT: f32 = 1e6;

const SIN: [f32; 4] = [
    f32::from_bits(0xbe2a_aaab),
    f32::from_bits(0x3c08_8889),
    f32::from_bits(0xb950_0d01),
    f32::from_bits(0x3638_ef1d),
];
const COS: [f32; 5] = [
    f32::from_bits(0xbf00_0000),
    f32::from_bits(0x3d2a_aaab),
    f32::from_bits(0xbab6_0b61),
    f32::from_bits(0x37d0_0d01),
    f32::from_bits(0xb493_f27e),
];
const EXP: [f32; 7] = [
    f32::from_bits(0x3f80_0000),
    f32::from_bits(0x3f00_0000),
    f32::from_bits(0x3e2a_aaab),
    f32::from_bits(0x3d2a_aaab),
    f32::from_bits(0x3c08_8889),
    f32::from_bits(0x3ab6_0b61),
    f32::from_bits(0x3950_0d01),
];
const LN: [f32; 5] = [
    f32::from_bits(0x3eaa_aaab),
    f32::from_bits(0x3e4c_cccd),
    f32::from_bits(0x3e12_4925),
    f32::from_bits(0x3de3_8e39),
    f32::from_bits(0x3dba_2e8c),
];
const ATAN: [f32; 9] = [
    f32::from_bits(0xbeaa_aaab),
    f32::from_bits(0x3e4c_cccd),
    f32::from_bits(0xbe12_4925),
    f32::from_bits(0x3de3_8e39),
    f32::from_bits(0xbdba_2e8c),
    f32::from_bits(0x3d9d_89d9),
    f32::from_bits(0xbd88_8889),
    f32::from_bits(0x3d70_f0f1),
    f32::from_bits(0xbd57_9436),
];

/// A subnormal value becomes a zero with its sign.
#[must_use]
pub(crate) fn flush(x: f32) -> f32 {
    let bits = x.to_bits();
    if bits & EXPONENT == 0 {
        f32::from_bits(bits & SIGN)
    } else {
        x
    }
}

fn add(a: f32, b: f32) -> f32 {
    flush(a + b)
}

fn sub(a: f32, b: f32) -> f32 {
    flush(a - b)
}

fn mul(a: f32, b: f32) -> f32 {
    flush(a * b)
}

fn negate(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ SIGN)
}

fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & !SIGN)
}

fn copy_sign(magnitude: f32, sign: f32) -> f32 {
    f32::from_bits((magnitude.to_bits() & !SIGN) | (sign.to_bits() & SIGN))
}

fn number(condition: bool) -> f32 {
    if condition { 1.0 } else { 0.0 }
}

fn select(otherwise: f32, then: f32, condition: bool) -> f32 {
    if condition { then } else { otherwise }
}

/// `p[0] + x · (p[1] + x · (…))`.
fn horner(x: f32, coefficients: &[f32]) -> f32 {
    let mut result = 0.0;
    let mut first = true;
    for coefficient in coefficients.iter().rev() {
        result = if first {
            *coefficient
        } else {
            add(*coefficient, mul(x, result))
        };
        first = false;
    }
    result
}

/// `2^k` for `k` in `[-126, 127]`.
fn power_of_two(k: i32) -> f32 {
    f32::from_bits(((k + 127) as u32) << 23)
}

/// `x · 2^k` for `k` in `[-252, 254]`, in two steps so that each factor is a normal number.
fn scale(x: f32, k: i32) -> f32 {
    let half = k / 2;
    mul(mul(x, power_of_two(half)), power_of_two(k - half))
}

/// The correctly rounded quotient; `0.0` for a zero divisor.
#[must_use]
pub(crate) fn divide(a: f32, b: f32) -> f32 {
    let (ua, ub) = (a.to_bits(), b.to_bits());
    let sign = (ua ^ ub) & SIGN;
    let (ea, eb) = ((ua >> 23) & 0xff, (ub >> 23) & 0xff);
    if eb == 0 {
        return 0.0;
    }
    if ea == 255 {
        let nan = ua & MANTISSA != 0 || eb == 255;
        return f32::from_bits(if nan { QUIET_NAN } else { sign | EXPONENT });
    }
    if eb == 255 {
        return f32::from_bits(if ub & MANTISSA != 0 { QUIET_NAN } else { sign });
    }
    if ea == 0 {
        return f32::from_bits(sign);
    }
    let mut dividend = (ua & MANTISSA) | IMPLICIT;
    let divisor = (ub & MANTISSA) | IMPLICIT;
    let mut exponent = ea as i32 - eb as i32 + 127;
    if dividend < divisor {
        dividend <<= 1;
        exponent -= 1;
    }
    let mut quotient = 0u32;
    for _ in 0..25 {
        quotient <<= 1;
        if dividend >= divisor {
            dividend -= divisor;
            quotient |= 1;
        }
        dividend <<= 1;
    }
    let mut mantissa = quotient >> 1;
    if quotient & 1 == 1 && (dividend != 0 || mantissa & 1 == 1) {
        mantissa += 1;
    }
    finish(sign, exponent, mantissa)
}

/// The bits of a result with a 24-bit mantissa that can carry into the exponent. A result below
/// the normal range is a zero; one above it is infinite.
fn finish(sign: u32, exponent: i32, mantissa: u32) -> f32 {
    if exponent <= 0 {
        return f32::from_bits(sign);
    }
    if exponent >= 255 {
        return f32::from_bits(sign | EXPONENT);
    }
    let bits = ((exponent as u32) << 23) + (mantissa - IMPLICIT);
    if bits >= EXPONENT {
        return f32::from_bits(sign | EXPONENT);
    }
    f32::from_bits(sign | bits)
}

/// The correctly rounded square root; NaN below zero.
#[must_use]
pub(crate) fn sqrt(x: f32) -> f32 {
    let bits = x.to_bits();
    if bits & !SIGN == 0 {
        return x;
    }
    if bits & SIGN != 0 {
        return f32::from_bits(QUIET_NAN);
    }
    if bits >= EXPONENT {
        return x;
    }
    let mut exponent = (bits >> 23) as i32 - 127;
    let mut mantissa = (bits & MANTISSA) | IMPLICIT;
    if exponent & 1 != 0 {
        mantissa <<= 1;
        exponent -= 1;
    }
    let mut root = 0u32;
    let mut remainder = 0u32;
    for group in 0..25i32 {
        let shift = 23 - 2 * group;
        let pair = if shift >= 0 {
            (mantissa >> shift) & 3
        } else if shift == -1 {
            (mantissa << 1) & 3
        } else {
            0
        };
        remainder = (remainder << 2) | pair;
        let trial = (root << 2) | 1;
        if remainder >= trial {
            remainder -= trial;
            root = (root << 1) | 1;
        } else {
            root <<= 1;
        }
    }
    let rounded = (root >> 1) + (root & 1);
    finish(0, (exponent >> 1) + 127, rounded)
}

/// The exact remainder of `a / b` with the sign of `a`; `0.0` for a zero divisor.
#[must_use]
pub(crate) fn remainder(a: f32, b: f32) -> f32 {
    let (ua, ub) = (a.to_bits(), b.to_bits());
    let sign = ua & SIGN;
    let (ax, bx) = (ua & !SIGN, ub & !SIGN);
    if bx == 0 {
        return 0.0;
    }
    if ax >= EXPONENT || bx > EXPONENT {
        return f32::from_bits(QUIET_NAN);
    }
    if ax < bx {
        return a;
    }
    if ax == bx {
        return f32::from_bits(sign);
    }
    let (mut ex, ey) = ((ax >> 23) as i32, (bx >> 23) as i32);
    let mut mx = (ax & MANTISSA) | IMPLICIT;
    let my = (bx & MANTISSA) | IMPLICIT;
    while ex > ey {
        if mx >= my {
            mx -= my;
            if mx == 0 {
                return f32::from_bits(sign);
            }
        }
        mx <<= 1;
        ex -= 1;
    }
    if mx >= my {
        mx -= my;
        if mx == 0 {
            return f32::from_bits(sign);
        }
    }
    while mx & IMPLICIT == 0 {
        mx <<= 1;
        ex -= 1;
    }
    if ex <= 0 {
        return f32::from_bits(sign);
    }
    f32::from_bits(sign | ((ex as u32) << 23) | (mx & MANTISSA))
}

/// The Molang reduction to `[-180, 180]`, then the quadrant `k` and the rest in `[-45, 45]`.
fn quadrant(degrees: f32) -> (i32, f32) {
    let mut reduced = remainder(degrees, 360.0);
    reduced = select(reduced, sub(reduced, 360.0), reduced > 180.0);
    reduced = select(reduced, add(reduced, 360.0), reduced < -180.0);
    let k = libm::floorf(add(mul(reduced, ONE_OVER_90), 0.5));
    let rest = sub(reduced, mul(k, 90.0));
    (k as i32, mul(rest, DEGREES_TO_RADIANS))
}

fn sin_poly(x: f32) -> f32 {
    let square = mul(x, x);
    add(x, mul(mul(x, square), horner(square, &SIN)))
}

fn cos_poly(x: f32) -> f32 {
    let square = mul(x, x);
    add(1.0, mul(square, horner(square, &COS)))
}

fn sin_degrees(degrees: f32) -> f32 {
    let (k, x) = quadrant(degrees);
    match k & 3 {
        0 => sin_poly(x),
        1 => cos_poly(x),
        2 => negate(sin_poly(x)),
        _ => negate(cos_poly(x)),
    }
}

fn cos_degrees(degrees: f32) -> f32 {
    let (k, x) = quadrant(degrees);
    match k & 3 {
        0 => cos_poly(x),
        1 => negate(sin_poly(x)),
        2 => negate(cos_poly(x)),
        _ => sin_poly(x),
    }
}

/// `e^r` for `|r| ≤ 0.35`.
fn exp_poly(r: f32) -> f32 {
    add(1.0, mul(r, horner(r, &EXP)))
}

fn exp(x: f32) -> f32 {
    let x = select(x, -104.0, x < -104.0);
    let x = select(x, 89.0, x > 89.0);
    let n = libm::floorf(add(mul(x, LOG2_E), 0.5));
    let r = sub(sub(x, mul(n, LN_2_HIGH)), mul(n, LN_2_LOW));
    scale(exp_poly(r), n as i32)
}

fn exp2(x: f32) -> f32 {
    let x = select(x, -150.0, x < -150.0);
    let x = select(x, 129.0, x > 129.0);
    let n = libm::floorf(add(x, 0.5));
    scale(exp_poly(mul(sub(x, n), LN_2)), n as i32)
}

fn ln(x: f32) -> f32 {
    let bits = x.to_bits();
    if bits & SIGN != 0 || bits & !SIGN == 0 || bits >= EXPONENT {
        return f32::from_bits(QUIET_NAN);
    }
    let mut exponent = (bits >> 23) as i32 - 127;
    let mut mantissa = f32::from_bits((bits & MANTISSA) | 0x3f80_0000);
    if mantissa > SQRT_2 {
        mantissa = mul(mantissa, 0.5);
        exponent += 1;
    }
    let f = sub(mantissa, 1.0);
    let s = divide(f, add(2.0, f));
    let square = mul(s, s);
    let tail = mul(square, horner(square, &LN));
    let twice = add(s, s);
    let ln_mantissa = add(twice, mul(twice, tail));
    let e = exponent as f32;
    add(mul(e, LN_2_HIGH), add(mul(e, LN_2_LOW), ln_mantissa))
}

fn pow(base: f32, exponent: f32) -> f32 {
    exp(mul(exponent, ln(base)))
}

/// `atan(t)` in radians for `t` in `[0, 1]`.
fn atan_unit(t: f32) -> f32 {
    let big = t > TAN_EIGHTH_PI;
    let u = select(t, divide(sub(t, 1.0), add(t, 1.0)), big);
    let square = mul(u, u);
    let p = add(u, mul(mul(u, square), horner(square, &ATAN)));
    select(p, add(QUARTER_PI, p), big)
}

fn atan_radians(x: f32) -> f32 {
    let a = abs(x);
    let inverse = a > 1.0;
    let r = atan_unit(select(a, divide(1.0, a), inverse));
    copy_sign(select(r, sub(HALF_PI, r), inverse), x)
}

fn atan2_radians(y: f32, x: f32) -> f32 {
    let (ax, ay) = (abs(x), abs(y));
    let swap = ay > ax;
    let large = select(ax, ay, swap);
    let small = select(ay, ax, swap);
    let r = atan_unit(divide(small, large));
    let r = select(r, sub(HALF_PI, r), swap);
    let r = select(r, sub(PI, r), x.to_bits() & SIGN != 0);
    copy_sign(r, y)
}

fn cosine_side(x: f32) -> f32 {
    sqrt(mul(sub(1.0, x), add(1.0, x)))
}

fn molang_min(a: f32, b: f32) -> f32 {
    select(a, b, b < a)
}

fn molang_max(a: f32, b: f32) -> f32 {
    select(a, b, b > a)
}

fn round(x: f32) -> f32 {
    let truncated = libm::truncf(x);
    let up = abs(sub(x, truncated)) >= 0.5;
    select(truncated, add(truncated, copy_sign(1.0, x)), up)
}

fn min_angle(angle: f32) -> f32 {
    sub(
        angle,
        mul(360.0, libm::floorf(divide(add(angle, 180.0), 360.0))),
    )
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    add(start, mul(sub(end, start), t))
}

fn bounce(u: f32) -> f32 {
    const N: f32 = 7.5625;
    let curve = |offset: u32, lift: f32| {
        let v = sub(u, f32::from_bits(offset));
        add(mul(N, mul(v, v)), lift)
    };
    if u < f32::from_bits(0x3eba_2e8c) {
        mul(N, mul(u, u))
    } else if u < f32::from_bits(0x3f3a_2e8c) {
        curve(0x3f0b_a2e9, 0.75)
    } else if u < f32::from_bits(0x3f68_ba2f) {
        curve(0x3f51_745d, 0.9375)
    } else {
        curve(0x3f74_5d17, 0.984_375)
    }
}

fn ease_in(shape: EaseShape, t: f32) -> f32 {
    match shape {
        EaseShape::Quad => mul(t, t),
        EaseShape::Cubic => mul(mul(t, t), t),
        EaseShape::Quart => mul(mul(mul(t, t), t), t),
        EaseShape::Quint => mul(mul(mul(mul(t, t), t), t), t),
        EaseShape::Sine => sub(1.0, cos_degrees(mul(90.0, t))),
        EaseShape::Expo => select(exp2(sub(mul(10.0, t), 10.0)), 0.0, t == 0.0),
        EaseShape::Circ => sub(1.0, sqrt(sub(1.0, mul(t, t)))),
        EaseShape::Back => sub(mul(2.701_58, mul(mul(t, t), t)), mul(1.701_58, mul(t, t))),
        EaseShape::Elastic => {
            let wave = mul(
                negate(exp2(sub(mul(10.0, t), 10.0))),
                sin_degrees(mul(sub(mul(10.0, t), 10.75), 120.0)),
            );
            select(select(wave, 1.0, t == 1.0), 0.0, t == 0.0)
        }
        EaseShape::Bounce => sub(1.0, bounce(sub(1.0, t))),
    }
}

fn ease(easing: Easing, t: f32) -> f32 {
    match easing.direction {
        EaseDirection::In => ease_in(easing.shape, t),
        EaseDirection::Out => sub(1.0, ease_in(easing.shape, sub(1.0, t))),
        EaseDirection::InOut => {
            if t < 0.5 {
                mul(ease_in(easing.shape, mul(2.0, t)), 0.5)
            } else {
                sub(1.0, mul(ease_in(easing.shape, sub(2.0, mul(2.0, t))), 0.5))
            }
        }
    }
}

#[must_use]
pub(crate) fn unary(function: UnaryFn, x: f32) -> f32 {
    match function {
        UnaryFn::Negate => negate(x),
        UnaryFn::Not => number(x == 0.0),
        UnaryFn::Truthy => number(x != 0.0),
        UnaryFn::Abs => abs(x),
        UnaryFn::Ceil => libm::ceilf(x),
        UnaryFn::Floor => libm::floorf(x),
        UnaryFn::Trunc => libm::truncf(x),
        UnaryFn::Round => round(x),
        UnaryFn::Sqrt => sqrt(x),
        UnaryFn::Exp => exp(x),
        UnaryFn::Ln => ln(x),
        UnaryFn::Sin => sin_degrees(x),
        UnaryFn::Cos => cos_degrees(x),
        UnaryFn::Asin => mul(atan2_radians(x, cosine_side(x)), RADIANS_TO_DEGREES),
        UnaryFn::Acos => mul(atan2_radians(cosine_side(x), x), RADIANS_TO_DEGREES),
        UnaryFn::Atan => mul(atan_radians(x), RADIANS_TO_DEGREES),
        UnaryFn::Sign => select(-1.0, 1.0, x > 0.0),
        UnaryFn::MinAngle => min_angle(x),
        UnaryFn::HermiteBlend => sub(mul(3.0, mul(x, x)), mul(2.0, mul(mul(x, x), x))),
    }
}

#[must_use]
pub(crate) fn binary(function: BinaryFn, a: f32, b: f32) -> f32 {
    match function {
        BinaryFn::Add => add(a, b),
        BinaryFn::Subtract => sub(a, b),
        BinaryFn::Multiply => mul(a, b),
        BinaryFn::Divide => divide(a, b),
        BinaryFn::Less => number(a < b),
        BinaryFn::LessEqual => number(a <= b),
        BinaryFn::Greater => number(a > b),
        BinaryFn::GreaterEqual => number(a >= b),
        BinaryFn::Equal => number(a == b),
        BinaryFn::NotEqual => number(a != b),
        BinaryFn::BothTruthy => number(a != 0.0 && b != 0.0),
        BinaryFn::EitherTruthy => number(a != 0.0 || b != 0.0),
        BinaryFn::ApproxEqual => number(abs(sub(a, b)) <= APPROX_EQ_TOLERANCE),
        BinaryFn::Min => molang_min(a, b),
        BinaryFn::Max => molang_max(a, b),
        BinaryFn::Mod => remainder(a, b),
        BinaryFn::Pow => pow(a, b),
        BinaryFn::Atan2 => mul(atan2_radians(a, b), RADIANS_TO_DEGREES),
        BinaryFn::CopySign => copy_sign(a, b),
    }
}

#[must_use]
pub(crate) fn ternary(function: TernaryFn, a: f32, b: f32, c: f32) -> f32 {
    match function {
        TernaryFn::Clamp => molang_min(molang_max(a, b), c),
        TernaryFn::Lerp => lerp(a, b, c),
        TernaryFn::InverseLerp => divide(sub(c, a), sub(b, a)),
        TernaryFn::LerpRotate => min_angle(add(a, mul(min_angle(sub(b, a)), c))),
        TernaryFn::InRange => number(b <= a && a <= c),
        TernaryFn::Ease(easing) => lerp(a, b, ease(easing, c)),
    }
}

/// The value that the pose pass samples: NaN gives `0.0`, and the rest is limited to
/// `[-1e6, 1e6]`.
#[must_use]
pub(crate) fn pose_clamp(x: f32) -> f32 {
    let bits = x.to_bits();
    if bits & EXPONENT == EXPONENT {
        if bits & MANTISSA != 0 {
            return 0.0;
        }
        return copy_sign(POSE_LIMIT, x);
    }
    let x = select(x, -POSE_LIMIT, x < -POSE_LIMIT);
    select(x, POSE_LIMIT, x > POSE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Normal numbers over the whole exponent range, with both signs.
    fn normals(count: usize, mut state: u64) -> impl Iterator<Item = f32> {
        core::iter::repeat_with(move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        })
        .map(|bits| f32::from_bits(bits as u32))
        .filter(|x| x.is_normal())
        .take(count)
    }

    fn same_or_both_subnormal(ours: f32, reference: f32) -> bool {
        ours.to_bits() == reference.to_bits()
            || (reference.is_subnormal() && ours == 0.0)
            || (reference.is_nan() && ours.is_nan())
    }

    #[test]
    fn integer_division_square_root_and_remainder_are_correctly_rounded() {
        let pairs = normals(200_000, 7).zip(normals(200_000, 11));
        for (a, b) in pairs {
            assert!(same_or_both_subnormal(divide(a, b), a / b), "{a:e} / {b:e}");
            assert!(
                same_or_both_subnormal(sqrt(a.abs()), libm::sqrtf(a.abs())),
                "sqrt {a:e}"
            );
            assert!(
                same_or_both_subnormal(remainder(a, b), libm::fmodf(a, b)),
                "{a:e} % {b:e}"
            );
        }
        for (a, b) in [
            (1.0f32, 3.0f32),
            (-0.0, 2.0),
            (6.0, 3.0),
            (1.0, 0.0),
            (0.0, 0.0),
        ] {
            let expected = if b == 0.0 { 0.0 } else { a / b };
            assert_eq!(divide(a, b).to_bits(), expected.to_bits(), "{a} / {b}");
        }
        assert_eq!(remainder(-7.0, 3.0), -1.0);
        assert_eq!(remainder(5.0, 0.0), 0.0);
        assert_eq!(sqrt(-0.0).to_bits(), (-0.0f32).to_bits());
    }

    #[test]
    fn trigonometry_stays_within_the_bound_of_the_specification() {
        let bound = 1.0 / 2048.0;
        let mut largest = 0.0f64;
        for step in -400_000..=400_000 {
            let degrees = step as f32 * 0.0137;
            let radians = f64::from(remainder(degrees, 360.0)).to_radians();
            for (ours, reference) in [
                (sin_degrees(degrees), libm::sin(radians)),
                (cos_degrees(degrees), libm::cos(radians)),
            ] {
                largest = largest.max((f64::from(ours) - reference).abs());
            }
        }
        assert!(largest <= bound, "trigonometry error {largest:e}");
        assert!(
            largest <= 2e-7,
            "the polynomials lost accuracy: {largest:e}"
        );
        assert_eq!(sin_degrees(90.0), 1.0);
        assert_eq!(cos_degrees(180.0), -1.0);
        assert_eq!(sin_degrees(3_600_090.0), 1.0);
    }

    fn relative_error(ours: f32, reference: f64) -> f64 {
        (f64::from(ours) - reference).abs() / reference.abs().max(f64::MIN_POSITIVE)
    }

    #[test]
    fn transcendental_functions_stay_within_a_few_rounding_steps() {
        let step = f64::from(f32::EPSILON);
        let mut worst = [0.0f64; 6];
        for x in normals(200_000, 3) {
            let magnitude = x.abs();
            if magnitude < 80.0 {
                worst[0] = worst[0].max(relative_error(exp(x), libm::exp(f64::from(x))));
            }
            worst[1] = worst[1].max(
                (f64::from(ln(magnitude)) - libm::log(f64::from(magnitude))).abs()
                    / libm::log(f64::from(magnitude)).abs().max(1.0),
            );
            worst[2] = worst[2].max(relative_error(atan_radians(x), libm::atan(f64::from(x))));
            let y = f32::from_bits(x.to_bits().rotate_left(7) & 0xbfff_ffff);
            if y.is_normal() {
                worst[3] = worst[3].max(
                    (f64::from(atan2_radians(y, x)) - libm::atan2(f64::from(y), f64::from(x)))
                        .abs(),
                );
            }
            let unit = x / f32::MAX.sqrt().max(magnitude);
            worst[4] = worst[4].max(
                (f64::from(atan2_radians(unit, cosine_side(unit))) - libm::asin(f64::from(unit)))
                    .abs(),
            );
            if (0.1..10.0).contains(&magnitude) {
                let power = libm::pow(f64::from(magnitude), 3.5);
                worst[5] = worst[5].max(relative_error(pow(magnitude, 3.5), power));
            }
        }
        let limits = [
            4.0 * step,
            4.0 * step,
            4.0 * step,
            4.0 * step,
            4.0 * step,
            32.0 * step,
        ];
        for (index, (error, limit)) in worst.iter().zip(limits).enumerate() {
            assert!(
                error <= &limit,
                "function {index}: error {error:e} over {limit:e}"
            );
        }
    }

    #[test]
    fn exact_compositions_match_the_scalar_formulas() {
        for x in normals(100_000, 5) {
            let x = x / 1e30_f32.max(1.0);
            let reference = libm::roundf(x);
            assert_eq!(round(x).to_bits(), reference.to_bits(), "round {x:e}");
            let angle = x - (360.0 * libm::floorf((x + 180.0) / 360.0));
            assert!(
                same_or_both_subnormal(min_angle(x), angle),
                "min_angle {x:e}"
            );
        }
        assert_eq!(round(-2.5), -3.0);
        assert_eq!(round(0.499_999_97), 0.0);
        assert_eq!(min_angle(180.0), -180.0);
    }
}
