#[must_use]
pub fn floor_to_i32(x: f32) -> i32 {
    x.floor() as i32
}

#[must_use]
pub fn ceil_to_i32(x: f32) -> i32 {
    x.ceil() as i32
}

#[must_use]
pub fn truncate_to_i32(x: f32) -> i32 {
    x as i32
}

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

#[must_use]
pub fn floor_mod(a: i32, b: i32) -> i32 {
    a.wrapping_sub(floor_div(a, b).wrapping_mul(b))
}
