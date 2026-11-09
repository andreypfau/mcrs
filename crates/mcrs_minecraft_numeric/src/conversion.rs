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
