// The Molang math of the generated channel functions. `gpu_math.rs` computes the same
// operations on the CPU; change both together.
//
// Metal compiles with fast math: it contracts `a * b + c` to an FMA, reassociates constants,
// ignores the sign of zero (`0.0 - x` becomes `-x`, `x * 0.0` becomes `0.0`) and flushes
// subnormal values. Each operand and each result of a float operation therefore goes through an
// integer operation with `molang_zero`, a zero that the compiler cannot see. The compiler then
// knows no operand value and has no chain of float operations to rewrite, and every backend gives
// the bits of the CPU twin. For the same reason a Boolean becomes a number with an integer
// select, and `molang_min` and `molang_max` compare opaque copies of their operands: a float
// `select` over a comparison of its own operands becomes a min or max instruction, which can
// drop the sign of a zero.

var<private> molang_zero: u32;

fn molang_flush(x: f32) -> f32 {
    let bits = bitcast<u32>(x) | molang_zero;
    return bitcast<f32>(select(bits, bits & 0x80000000u, (bits & 0x7f800000u) == 0u));
}

fn molang_constant(bits: u32) -> f32 {
    return molang_flush(bitcast<f32>(bits | molang_zero));
}

fn molang_opaque(x: f32) -> f32 { return bitcast<f32>(bitcast<u32>(x) | molang_zero); }
fn molang_add(a: f32, b: f32) -> f32 { return molang_flush(molang_opaque(a) + molang_opaque(b)); }
fn molang_sub(a: f32, b: f32) -> f32 { return molang_flush(molang_opaque(a) - molang_opaque(b)); }
fn molang_mul(a: f32, b: f32) -> f32 { return molang_flush(molang_opaque(a) * molang_opaque(b)); }
fn molang_negate(x: f32) -> f32 { return bitcast<f32>(bitcast<u32>(x) ^ 0x80000000u); }
fn molang_abs(x: f32) -> f32 { return bitcast<f32>(bitcast<u32>(x) & 0x7fffffffu); }

fn molang_copy_sign(magnitude: f32, sign: f32) -> f32 {
    return bitcast<f32>((bitcast<u32>(magnitude) & 0x7fffffffu) | (bitcast<u32>(sign) & 0x80000000u));
}

fn molang_number(condition: bool) -> f32 {
    return bitcast<f32>(select(0u, 0x3f800000u, condition) | molang_zero);
}
fn molang_is_truthy(x: f32) -> bool { return (bitcast<u32>(x) & 0x7fffffffu) != 0u; }
fn molang_not(x: f32) -> f32 { return molang_number(!molang_is_truthy(x)); }
fn molang_truthy(x: f32) -> f32 { return molang_number(molang_is_truthy(x)); }
fn molang_sign(x: f32) -> f32 {
    return bitcast<f32>(select(0xbf800000u, 0x3f800000u, x > 0.0) | molang_zero);
}
fn molang_both_truthy(a: f32, b: f32) -> f32 {
    return molang_number(molang_is_truthy(a) && molang_is_truthy(b));
}
fn molang_either_truthy(a: f32, b: f32) -> f32 {
    return molang_number(molang_is_truthy(a) || molang_is_truthy(b));
}

fn molang_power_of_two(k: i32) -> f32 { return bitcast<f32>(u32(k + 127) << 23u); }

fn molang_scale(x: f32, k: i32) -> f32 {
    let half = k / 2;
    return molang_mul(molang_mul(x, molang_power_of_two(half)), molang_power_of_two(k - half));
}

fn molang_finish(sign: u32, exponent: i32, mantissa: u32) -> f32 {
    if (exponent <= 0) { return bitcast<f32>(sign); }
    if (exponent >= 255) { return bitcast<f32>(sign | 0x7f800000u); }
    let bits = (u32(exponent) << 23u) + (mantissa - 0x00800000u);
    if (bits >= 0x7f800000u) { return bitcast<f32>(sign | 0x7f800000u); }
    return bitcast<f32>(sign | bits);
}

fn molang_divide(a: f32, b: f32) -> f32 {
    let ua = bitcast<u32>(a);
    let ub = bitcast<u32>(b);
    let sign = (ua ^ ub) & 0x80000000u;
    let ea = (ua >> 23u) & 0xffu;
    let eb = (ub >> 23u) & 0xffu;
    if (eb == 0u) { return 0.0; }
    if (ea == 255u) {
        let nan = (ua & 0x007fffffu) != 0u || eb == 255u;
        return bitcast<f32>(select(sign | 0x7f800000u, 0x7fc00000u, nan));
    }
    if (eb == 255u) {
        return bitcast<f32>(select(sign, 0x7fc00000u, (ub & 0x007fffffu) != 0u));
    }
    if (ea == 0u) { return bitcast<f32>(sign); }
    var dividend = (ua & 0x007fffffu) | 0x00800000u;
    let divisor = (ub & 0x007fffffu) | 0x00800000u;
    var exponent = i32(ea) - i32(eb) + 127;
    if (dividend < divisor) {
        dividend = dividend << 1u;
        exponent = exponent - 1;
    }
    var quotient = 0u;
    for (var i = 0u; i < 25u; i = i + 1u) {
        quotient = quotient << 1u;
        if (dividend >= divisor) {
            dividend = dividend - divisor;
            quotient = quotient | 1u;
        }
        dividend = dividend << 1u;
    }
    var mantissa = quotient >> 1u;
    if ((quotient & 1u) == 1u && (dividend != 0u || (mantissa & 1u) == 1u)) {
        mantissa = mantissa + 1u;
    }
    return molang_finish(sign, exponent, mantissa);
}

fn molang_sqrt(x: f32) -> f32 {
    let bits = bitcast<u32>(x);
    if ((bits & 0x7fffffffu) == 0u) { return x; }
    if ((bits & 0x80000000u) != 0u) { return bitcast<f32>(0x7fc00000u); }
    if (bits >= 0x7f800000u) { return x; }
    var exponent = i32(bits >> 23u) - 127;
    var mantissa = (bits & 0x007fffffu) | 0x00800000u;
    if ((exponent & 1) != 0) {
        mantissa = mantissa << 1u;
        exponent = exponent - 1;
    }
    var root = 0u;
    var remainder = 0u;
    for (var group = 0i; group < 25i; group = group + 1i) {
        let shift = 23i - 2i * group;
        var pair = 0u;
        if (shift >= 0i) {
            pair = (mantissa >> u32(shift)) & 3u;
        } else if (shift == -1i) {
            pair = (mantissa << 1u) & 3u;
        }
        remainder = (remainder << 2u) | pair;
        let trial = (root << 2u) | 1u;
        if (remainder >= trial) {
            remainder = remainder - trial;
            root = (root << 1u) | 1u;
        } else {
            root = root << 1u;
        }
    }
    let rounded = (root >> 1u) + (root & 1u);
    return molang_finish(0u, (exponent >> 1u) + 127, rounded);
}

fn molang_mod(a: f32, b: f32) -> f32 {
    let ua = bitcast<u32>(a);
    let ub = bitcast<u32>(b);
    let sign = ua & 0x80000000u;
    let ax = ua & 0x7fffffffu;
    let bx = ub & 0x7fffffffu;
    if (bx == 0u) { return 0.0; }
    if (ax >= 0x7f800000u || bx > 0x7f800000u) { return bitcast<f32>(0x7fc00000u); }
    if (ax < bx) { return a; }
    if (ax == bx) { return bitcast<f32>(sign); }
    var ex = i32(ax >> 23u);
    let ey = i32(bx >> 23u);
    var mx = (ax & 0x007fffffu) | 0x00800000u;
    let my = (bx & 0x007fffffu) | 0x00800000u;
    loop {
        if (ex <= ey) { break; }
        if (mx >= my) {
            mx = mx - my;
            if (mx == 0u) { return bitcast<f32>(sign); }
        }
        mx = mx << 1u;
        ex = ex - 1;
    }
    if (mx >= my) {
        mx = mx - my;
        if (mx == 0u) { return bitcast<f32>(sign); }
    }
    loop {
        if ((mx & 0x00800000u) != 0u) { break; }
        mx = mx << 1u;
        ex = ex - 1;
    }
    if (ex <= 0) { return bitcast<f32>(sign); }
    return bitcast<f32>(sign | (u32(ex) << 23u) | (mx & 0x007fffffu));
}

struct MolangQuadrant {
    k: i32,
    x: f32,
}

fn molang_quadrant(degrees: f32) -> MolangQuadrant {
    var reduced = molang_mod(degrees, 360.0);
    reduced = select(reduced, molang_sub(reduced, 360.0), reduced > 180.0);
    reduced = select(reduced, molang_add(reduced, 360.0), reduced < -180.0);
    let k = floor(molang_add(molang_mul(reduced, bitcast<f32>(0x3c360b61u)), 0.5));
    let rest = molang_sub(reduced, molang_mul(k, 90.0));
    return MolangQuadrant(i32(k), molang_mul(rest, bitcast<f32>(0x3c8efa35u)));
}

fn molang_sin_poly(x: f32) -> f32 {
    let s = molang_mul(x, x);
    var p = bitcast<f32>(0x3638ef1du);
    p = molang_add(bitcast<f32>(0xb9500d01u), molang_mul(s, p));
    p = molang_add(bitcast<f32>(0x3c088889u), molang_mul(s, p));
    p = molang_add(bitcast<f32>(0xbe2aaaabu), molang_mul(s, p));
    return molang_add(x, molang_mul(molang_mul(x, s), p));
}

fn molang_cos_poly(x: f32) -> f32 {
    let s = molang_mul(x, x);
    var p = bitcast<f32>(0xb493f27eu);
    p = molang_add(bitcast<f32>(0x37d00d01u), molang_mul(s, p));
    p = molang_add(bitcast<f32>(0xbab60b61u), molang_mul(s, p));
    p = molang_add(bitcast<f32>(0x3d2aaaabu), molang_mul(s, p));
    p = molang_add(bitcast<f32>(0xbf000000u), molang_mul(s, p));
    return molang_add(1.0, molang_mul(s, p));
}

fn molang_sin(degrees: f32) -> f32 {
    let q = molang_quadrant(degrees);
    switch (q.k & 3) {
        case 0: { return molang_sin_poly(q.x); }
        case 1: { return molang_cos_poly(q.x); }
        case 2: { return molang_negate(molang_sin_poly(q.x)); }
        default: { return molang_negate(molang_cos_poly(q.x)); }
    }
}

fn molang_cos(degrees: f32) -> f32 {
    let q = molang_quadrant(degrees);
    switch (q.k & 3) {
        case 0: { return molang_cos_poly(q.x); }
        case 1: { return molang_negate(molang_sin_poly(q.x)); }
        case 2: { return molang_negate(molang_cos_poly(q.x)); }
        default: { return molang_sin_poly(q.x); }
    }
}

fn molang_exp_poly(r: f32) -> f32 {
    var p = bitcast<f32>(0x39500d01u);
    p = molang_add(bitcast<f32>(0x3ab60b61u), molang_mul(r, p));
    p = molang_add(bitcast<f32>(0x3c088889u), molang_mul(r, p));
    p = molang_add(bitcast<f32>(0x3d2aaaabu), molang_mul(r, p));
    p = molang_add(bitcast<f32>(0x3e2aaaabu), molang_mul(r, p));
    p = molang_add(bitcast<f32>(0x3f000000u), molang_mul(r, p));
    p = molang_add(bitcast<f32>(0x3f800000u), molang_mul(r, p));
    return molang_add(1.0, molang_mul(r, p));
}

fn molang_exp(value: f32) -> f32 {
    var x = select(value, -104.0, value < -104.0);
    x = select(x, 89.0, x > 89.0);
    let n = floor(molang_add(molang_mul(x, bitcast<f32>(0x3fb8aa3bu)), 0.5));
    let r = molang_sub(
        molang_sub(x, molang_mul(n, bitcast<f32>(0x3f317200u))),
        molang_mul(n, bitcast<f32>(0x35bfbe8eu)),
    );
    return molang_scale(molang_exp_poly(r), i32(n));
}

fn molang_exp2(value: f32) -> f32 {
    var x = select(value, -150.0, value < -150.0);
    x = select(x, 129.0, x > 129.0);
    let n = floor(molang_add(x, 0.5));
    return molang_scale(molang_exp_poly(molang_mul(molang_sub(x, n), bitcast<f32>(0x3f317218u))), i32(n));
}

fn molang_ln(x: f32) -> f32 {
    let bits = bitcast<u32>(x);
    if ((bits & 0x80000000u) != 0u || (bits & 0x7fffffffu) == 0u || bits >= 0x7f800000u) {
        return bitcast<f32>(0x7fc00000u);
    }
    var exponent = i32(bits >> 23u) - 127;
    var mantissa = bitcast<f32>((bits & 0x007fffffu) | 0x3f800000u);
    if (mantissa > bitcast<f32>(0x3fb504f3u)) {
        mantissa = molang_mul(mantissa, 0.5);
        exponent = exponent + 1;
    }
    let f = molang_sub(mantissa, 1.0);
    let s = molang_divide(f, molang_add(2.0, f));
    let square = molang_mul(s, s);
    var p = bitcast<f32>(0x3dba2e8cu);
    p = molang_add(bitcast<f32>(0x3de38e39u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3e124925u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3e4ccccdu), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3eaaaaabu), molang_mul(square, p));
    let tail = molang_mul(square, p);
    let twice = molang_add(s, s);
    let ln_mantissa = molang_add(twice, molang_mul(twice, tail));
    let e = f32(exponent);
    return molang_add(
        molang_mul(e, bitcast<f32>(0x3f317200u)),
        molang_add(molang_mul(e, bitcast<f32>(0x35bfbe8eu)), ln_mantissa),
    );
}

fn molang_pow(base: f32, exponent: f32) -> f32 {
    return molang_exp(molang_mul(exponent, molang_ln(base)));
}

fn molang_atan_unit(t: f32) -> f32 {
    let big = t > bitcast<f32>(0x3ed413cdu);
    let u = select(t, molang_divide(molang_sub(t, 1.0), molang_add(t, 1.0)), big);
    let square = molang_mul(u, u);
    var p = bitcast<f32>(0xbd579436u);
    p = molang_add(bitcast<f32>(0x3d70f0f1u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0xbd888889u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3d9d89d9u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0xbdba2e8cu), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3de38e39u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0xbe124925u), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0x3e4ccccdu), molang_mul(square, p));
    p = molang_add(bitcast<f32>(0xbeaaaaabu), molang_mul(square, p));
    let result = molang_add(u, molang_mul(molang_mul(u, square), p));
    return select(result, molang_add(bitcast<f32>(0x3f490fdbu), result), big);
}

fn molang_atan_radians(x: f32) -> f32 {
    let a = molang_abs(x);
    let inverse = a > 1.0;
    let r = molang_atan_unit(select(a, molang_divide(1.0, a), inverse));
    return molang_copy_sign(select(r, molang_sub(bitcast<f32>(0x3fc90fdbu), r), inverse), x);
}

fn molang_atan2_radians(y: f32, x: f32) -> f32 {
    let ax = molang_abs(x);
    let ay = molang_abs(y);
    let swap = molang_opaque(ay) > molang_opaque(ax);
    let large = select(ax, ay, swap);
    let small = select(ay, ax, swap);
    var r = molang_atan_unit(molang_divide(small, large));
    r = select(r, molang_sub(bitcast<f32>(0x3fc90fdbu), r), swap);
    r = select(r, molang_sub(bitcast<f32>(0x40490fdbu), r), (bitcast<u32>(x) & 0x80000000u) != 0u);
    return molang_copy_sign(r, y);
}

fn molang_cosine_side(x: f32) -> f32 {
    return molang_sqrt(molang_mul(molang_sub(1.0, x), molang_add(1.0, x)));
}

fn molang_degrees(radians: f32) -> f32 { return molang_mul(radians, bitcast<f32>(0x42652ee0u)); }

fn molang_asin(x: f32) -> f32 { return molang_degrees(molang_atan2_radians(x, molang_cosine_side(x))); }
fn molang_acos(x: f32) -> f32 { return molang_degrees(molang_atan2_radians(molang_cosine_side(x), x)); }
fn molang_atan(x: f32) -> f32 { return molang_degrees(molang_atan_radians(x)); }
fn molang_atan2(y: f32, x: f32) -> f32 { return molang_degrees(molang_atan2_radians(y, x)); }

fn molang_min(a: f32, b: f32) -> f32 { return select(a, b, molang_opaque(b) < molang_opaque(a)); }
fn molang_max(a: f32, b: f32) -> f32 { return select(a, b, molang_opaque(b) > molang_opaque(a)); }

fn molang_round(x: f32) -> f32 {
    let truncated = trunc(x);
    let up = molang_abs(molang_sub(x, truncated)) >= 0.5;
    return select(truncated, molang_add(truncated, molang_copy_sign(1.0, x)), up);
}

fn molang_min_angle(angle: f32) -> f32 {
    return molang_sub(angle, molang_mul(360.0, floor(molang_divide(molang_add(angle, 180.0), 360.0))));
}

fn molang_hermite_blend(x: f32) -> f32 {
    return molang_sub(molang_mul(3.0, molang_mul(x, x)), molang_mul(2.0, molang_mul(molang_mul(x, x), x)));
}

fn molang_lerp(start: f32, end: f32, t: f32) -> f32 {
    return molang_add(start, molang_mul(molang_sub(end, start), t));
}

fn molang_inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    return molang_divide(molang_sub(v, a), molang_sub(b, a));
}

fn molang_lerp_rotate(a: f32, b: f32, t: f32) -> f32 {
    return molang_min_angle(molang_add(a, molang_mul(molang_min_angle(molang_sub(b, a)), t)));
}

fn molang_clamp(v: f32, lo: f32, hi: f32) -> f32 { return molang_min(molang_max(v, lo), hi); }
fn molang_in_range(v: f32, lo: f32, hi: f32) -> f32 { return molang_number(lo <= v && v <= hi); }

fn molang_approx_eq(a: f32, b: f32) -> f32 {
    return molang_number(molang_abs(molang_sub(a, b)) <= bitcast<f32>(0x33d6bf95u));
}

fn molang_bounce_curve(u: f32, offset: u32, lift: f32) -> f32 {
    let v = molang_sub(u, bitcast<f32>(offset));
    return molang_add(molang_mul(7.5625, molang_mul(v, v)), lift);
}

fn molang_bounce(u: f32) -> f32 {
    if (u < bitcast<f32>(0x3eba2e8cu)) { return molang_mul(7.5625, molang_mul(u, u)); }
    if (u < bitcast<f32>(0x3f3a2e8cu)) { return molang_bounce_curve(u, 0x3f0ba2e9u, 0.75); }
    if (u < bitcast<f32>(0x3f68ba2fu)) { return molang_bounce_curve(u, 0x3f51745du, 0.9375); }
    return molang_bounce_curve(u, 0x3f745d17u, 0.984375);
}

// The shapes in the order of the shape index of `molang_ease_in`.
fn molang_ease_in(shape: u32, t: f32) -> f32 {
    switch (shape) {
        case 0u: { return molang_mul(t, t); }
        case 1u: { return molang_mul(molang_mul(t, t), t); }
        case 2u: { return molang_mul(molang_mul(molang_mul(t, t), t), t); }
        case 3u: { return molang_mul(molang_mul(molang_mul(molang_mul(t, t), t), t), t); }
        case 4u: { return molang_sub(1.0, molang_cos(molang_mul(90.0, t))); }
        case 5u: { return select(molang_exp2(molang_sub(molang_mul(10.0, t), 10.0)), 0.0, t == 0.0); }
        case 6u: { return molang_sub(1.0, molang_sqrt(molang_sub(1.0, molang_mul(t, t)))); }
        case 7u: {
            return molang_sub(
                molang_mul(2.70158, molang_mul(molang_mul(t, t), t)),
                molang_mul(1.70158, molang_mul(t, t)),
            );
        }
        case 8u: {
            let wave = molang_mul(
                molang_negate(molang_exp2(molang_sub(molang_mul(10.0, t), 10.0))),
                molang_sin(molang_mul(molang_sub(molang_mul(10.0, t), 10.75), 120.0)),
            );
            return select(select(wave, 1.0, t == 1.0), 0.0, t == 0.0);
        }
        default: { return molang_sub(1.0, molang_bounce(molang_sub(1.0, t))); }
    }
}

// `direction` is 0 for `in`, 1 for `out` and 2 for `in_out`.
fn molang_ease(direction: u32, shape: u32, start: f32, end: f32, t: f32) -> f32 {
    var progress: f32;
    if (direction == 0u) {
        progress = molang_ease_in(shape, t);
    } else if (direction == 1u) {
        progress = molang_sub(1.0, molang_ease_in(shape, molang_sub(1.0, t)));
    } else if (t < 0.5) {
        progress = molang_mul(molang_ease_in(shape, molang_mul(2.0, t)), 0.5);
    } else {
        progress = molang_sub(1.0, molang_mul(molang_ease_in(shape, molang_sub(2.0, molang_mul(2.0, t))), 0.5));
    }
    return molang_lerp(start, end, progress);
}

// NaN gives 0; the rest is limited to [-1e6, 1e6], with integer tests that fast math keeps.
fn molang_pose_clamp(x: f32) -> f32 {
    let bits = bitcast<u32>(x);
    if ((bits & 0x7f800000u) == 0x7f800000u) {
        if ((bits & 0x007fffffu) != 0u) { return 0.0; }
        return molang_copy_sign(1e6, x);
    }
    let low = select(x, -1e6, x < -1e6);
    return select(low, 1e6, low > 1e6);
}
