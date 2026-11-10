/// The median of `values`, the mean of the middle two for an even count; sorts in place.
pub fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        return values.get(middle).copied();
    }
    let low = values.get(middle.checked_sub(1)?)?;
    let high = values.get(middle)?;
    Some(f64::midpoint(*low, *high))
}

/// `max - min` of `values`, `None` when it is empty.
pub fn spread(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut bounds: Option<(f64, f64)> = None;
    for value in values {
        bounds = Some(match bounds {
            Some((low, high)) => (low.min(value), high.max(value)),
            None => (value, value),
        });
    }
    bounds.map(|(low, high)| high - low)
}
