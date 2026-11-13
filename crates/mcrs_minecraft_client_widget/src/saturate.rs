pub(crate) fn saturate(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

pub(crate) fn round(value: f64) -> i64 {
    libm::floor(value + 0.5) as i64
}
