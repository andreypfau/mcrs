use alloc::string::String;
use core::fmt::{self, Write};
use core::str::FromStr;

struct Fixed<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Fixed<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        self.bytes
            .get(..self.len)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
            .unwrap_or("")
    }

    fn pop(&mut self) {
        self.len = self.len.saturating_sub(1);
    }
}

impl<const N: usize> Write for Fixed<N> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

struct Digits {
    text: Fixed<24>,
    exponent: i32,
}

impl Digits {
    fn parse(scientific: &str) -> Self {
        let (mantissa, exponent) = scientific.split_once('e').unwrap_or((scientific, "0"));
        let mut text = Fixed::new();
        for digit in mantissa.chars().filter(char::is_ascii_digit) {
            _ = text.write_char(digit);
        }
        while text.len > 1 && text.as_str().ends_with('0') {
            text.pop();
        }
        Self {
            text,
            exponent: exponent.parse().unwrap_or(0),
        }
    }
}

/// Java's `Double.toString`: the shortest digits that round-trip but never
/// fewer than two, plain notation in `[1e-3, 1e7)` and `d.dddE±n` outside.
pub fn java_double(v: f64) -> String {
    let mut out = String::new();
    _ = write_java_number(&mut out, v);
    out
}

/// Java's `Float.toString`, with the same notation as [`java_double`].
pub fn java_float(v: f32) -> String {
    let mut out = String::new();
    _ = write_java_number(&mut out, v);
    out
}

pub(crate) fn write_java_number<T>(out: &mut impl Write, v: T) -> fmt::Result
where
    T: fmt::LowerExp + FromStr + PartialEq + Copy + Into<f64>,
{
    let mut shortest = Fixed::<40>::new();
    let mut two_digits = Fixed::<16>::new();
    write!(shortest, "{v:e}")?;
    write!(two_digits, "{v:.1e}")?;
    let mut digits = Digits::parse(shortest.as_str());
    if digits.text.len == 1 && two_digits.as_str().parse::<T>().ok() == Some(v) {
        digits = Digits::parse(two_digits.as_str());
    }
    write_number(out, v.into(), &digits)
}

fn write_zeros(out: &mut impl Write, count: usize) -> fmt::Result {
    for _ in 0..count {
        out.write_char('0')?;
    }
    Ok(())
}

fn write_number(out: &mut impl Write, v: f64, parsed: &Digits) -> fmt::Result {
    if v.is_nan() {
        return out.write_str("NaN");
    }
    if v.is_infinite() {
        return out.write_str(if v > 0.0 { "Infinity" } else { "-Infinity" });
    }
    let digits = parsed.text.as_str();
    let exponent = parsed.exponent;
    if v.is_sign_negative() {
        out.write_char('-')?;
    }
    let magnitude = v.abs();
    if magnitude == 0.0 {
        out.write_str("0.0")
    } else if (1e-3..1e7).contains(&magnitude) {
        if exponent >= 0 {
            let point = exponent as usize + 1;
            match digits.split_at_checked(point) {
                Some((whole, fraction)) if !fraction.is_empty() => {
                    out.write_str(whole)?;
                    out.write_char('.')?;
                    out.write_str(fraction)
                }
                _ => {
                    out.write_str(digits)?;
                    write_zeros(out, point.saturating_sub(digits.len()))?;
                    out.write_str(".0")
                }
            }
        } else {
            out.write_str("0.")?;
            write_zeros(out, exponent.unsigned_abs().saturating_sub(1) as usize)?;
            out.write_str(digits)
        }
    } else {
        let (head, tail) = digits.split_at_checked(1).unwrap_or((digits, ""));
        out.write_str(head)?;
        out.write_char('.')?;
        out.write_str(if tail.is_empty() { "0" } else { tail })?;
        write!(out, "E{exponent}")
    }
}
