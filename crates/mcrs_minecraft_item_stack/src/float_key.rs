// Equal keys mean equal floats for a component: every NaN gives one key, and `0.0` and `-0.0`
// give two keys.
pub(crate) fn of_f32(value: f32) -> u32 {
    if value.is_nan() {
        f32::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

pub(crate) fn of_f64(value: f64) -> u64 {
    if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}
