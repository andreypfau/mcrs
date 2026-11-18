use alloc::format;
use alloc::string::String;

pub(crate) fn decimal(value: f64) -> String {
    let mut text = format!("{value:.2}");
    let kept = text.trim_end_matches('0').trim_end_matches('.').len();
    text.truncate(kept);
    if text == "-0" {
        text.clear();
        text.push('0');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shown_number_has_at_most_two_digits_and_no_trailing_zero() {
        let cases: [(f64, &str); 9] = [
            (6.0, "6"),
            (4.0 - 2.400_000_095_367_431_6, "1.6"),
            (0.1, "0.1"),
            (12.345_6, "12.35"),
            (-0.5, "-0.5"),
            (-0.001, "0"),
            (1_000_000.0, "1000000"),
            (0.07, "0.07"),
            (1e18, "1000000000000000000"),
        ];
        for (value, text) in cases {
            assert_eq!(decimal(value), text, "{value}");
        }
    }
}
