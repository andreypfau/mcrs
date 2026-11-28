pub(crate) fn lerp(t: f32, start: i32, end: i32) -> f32 {
    let start = start as f32;
    start + t * (end as f32 - start)
}

pub(crate) fn round(value: f32) -> i32 {
    libm::floorf(value + 0.5) as i32
}

pub(crate) fn even_split(total: i32, parts: i32, index: i32) -> i32 {
    let (Some(quotient), Some(remainder)) = (total.checked_div(parts), total.checked_rem(parts))
    else {
        return 0;
    };
    let share =
        |index: i32| (i64::from(index) * i64::from(remainder)).checked_div(i64::from(parts));
    if remainder >= 0 && share(index) > share(index.saturating_sub(1)) {
        quotient.saturating_add(1)
    } else {
        quotient
    }
}
