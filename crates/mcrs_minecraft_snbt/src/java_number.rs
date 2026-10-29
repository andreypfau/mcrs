use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::Write;

/// Java's `Double.toString`: the shortest digits that round-trip but never
/// fewer than two, plain notation in `[1e-3, 1e7)` and `d.dddE±n` outside.
pub fn java_double(v: f64) -> String {
    let two_digits = format!("{v:.1e}");
    java_number(v, format!("{v:e}"), two_digits.parse() == Ok(v), two_digits)
}

pub fn java_float(v: f32) -> String {
    let two_digits = format!("{v:.1e}");
    java_number(
        v as f64,
        format!("{v:e}"),
        two_digits.parse() == Ok(v),
        two_digits,
    )
}

fn java_number(
    v: f64,
    shortest: String,
    two_digits_round_trip: bool,
    two_digits: String,
) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let split = |text: &str| -> (String, i32) {
        let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
        let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        while digits.len() > 1 && digits.ends_with('0') {
            digits.pop();
        }
        (digits, exponent.parse().unwrap_or(0))
    };
    let (mut digits, mut exponent) = split(&shortest);
    if digits.len() == 1 && two_digits_round_trip {
        (digits, exponent) = split(&two_digits);
    }
    let mut out = String::new();
    if v.is_sign_negative() {
        out.push('-');
    }
    let magnitude = v.abs();
    if magnitude == 0.0 {
        out.push_str("0.0");
    } else if (1e-3..1e7).contains(&magnitude) {
        if exponent >= 0 {
            let point = exponent as usize + 1;
            if digits.len() <= point {
                out.push_str(&digits);
                out.extend(core::iter::repeat_n('0', point - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            }
        } else {
            out.push_str("0.");
            out.extend(core::iter::repeat_n('0', (-exponent - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        _ = write!(out, "E{exponent}");
    }
    out
}
