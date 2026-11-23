use mcrs_minecraft_snbt::java_double;

const SHOWN_DECIMALS: usize = 2;

pub(crate) fn scale_text(scale: f64) -> String {
    let printed = java_double(scale);
    let (sign, magnitude) = match printed.strip_prefix('-') {
        Some(magnitude) => ("-", magnitude),
        None => ("", printed.as_str()),
    };
    let Some(Decimal { digits, point }) = Decimal::read(magnitude) else {
        return printed;
    };
    let kept = usize::try_from(point.saturating_add(SHOWN_DECIMALS as isize)).ok();
    let padded = digits.iter().copied().chain(core::iter::repeat(0));
    let mut rounded: Vec<u8> = padded.take(kept.unwrap_or_default()).collect();
    let next = kept.and_then(|index| digits.get(index)).copied();
    if next.is_some_and(|digit| digit >= 5) {
        round_up(&mut rounded);
    }
    while rounded.len() <= SHOWN_DECIMALS {
        rounded.insert(0, 0);
    }
    let split = rounded.len().saturating_sub(SHOWN_DECIMALS);
    let (whole, fraction) = rounded.split_at(split);
    let leading_zeros = whole.iter().take_while(|digit| **digit == 0).count();
    let whole = whole
        .get(leading_zeros.min(whole.len().saturating_sub(1))..)
        .unwrap_or_default();
    let mut text = String::from(sign);
    text.extend(whole.iter().map(|digit| char::from(b'0' + digit)));
    text.push('.');
    text.extend(fraction.iter().map(|digit| char::from(b'0' + digit)));
    text
}

struct Decimal {
    digits: Vec<u8>,
    point: isize,
}

impl Decimal {
    fn read(text: &str) -> Option<Decimal> {
        let (mantissa, exponent) = match text.split_once('E') {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<isize>().ok()?),
            None => (text, 0),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits = whole
            .bytes()
            .chain(fraction.bytes())
            .map(|byte| byte.is_ascii_digit().then(|| byte - b'0'))
            .collect::<Option<Vec<u8>>>()?;
        let point = isize::try_from(whole.len()).ok()?.checked_add(exponent)?;
        Some(Decimal { digits, point })
    }
}

fn round_up(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit < 9 {
            *digit += 1;
            return;
        }
        *digit = 0;
    }
    digits.insert(0, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_text_rounds_the_printed_decimal_half_away_from_zero() {
        let cases = [
            (10.0, "10.00"),
            (0.125, "0.13"),
            (1.005, "1.01"),
            (2.675, "2.68"),
            (1.115, "1.12"),
            (0.005, "0.01"),
            (-0.005, "-0.01"),
            (-0.004, "-0.00"),
            (-0.0, "-0.00"),
            (1_234_567.891, "1234567.89"),
            (10_000_000_000.0, "10000000000.00"),
            (0.999, "1.00"),
            (1.0E-5, "0.00"),
        ];
        for (scale, text) in cases {
            assert_eq!(scale_text(scale), text, "{scale}");
        }
    }
}
