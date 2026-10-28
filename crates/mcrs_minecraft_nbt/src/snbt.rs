use std::fmt::Write;

use crate::compound::NbtCompound;
use crate::tag::NbtTag;

mod build;
mod error;
mod read;
mod text;

pub use error::{IntegerType, SnbtError, SnbtErrorKind, Span};

use build::{Tags, Validate};
use read::{Reader, Shape};

/// One tag from `start`, the cursor after it (SNBT-ARG-001). The text after the tag is not read.
pub fn read_tag(input: &str, start: usize) -> Result<(NbtTag, usize), SnbtError> {
    let mut reader = Reader::new(input, start, Tags);
    let (tag, _) = reader.read()?;
    Ok((tag, reader.position()))
}

/// One compound from `start`, the cursor after it (SNBT-ARG-002).
pub fn read_compound(input: &str, start: usize) -> Result<(NbtCompound, usize), SnbtError> {
    let mut reader = Reader::new(input, start, Tags);
    match reader.read()? {
        (NbtTag::Compound(compound), _) => Ok((compound, reader.position())),
        _ => Err(expected_compound(reader.position())),
    }
}

/// The read of [`read_compound`] without its result: the cursor after the compound. It
/// allocates nothing.
pub fn check_compound(input: &str, start: usize) -> Result<usize, SnbtError> {
    let mut reader = Reader::new(input, start, Validate);
    match reader.read()? {
        ((), Shape::Compound) => Ok(reader.position()),
        _ => Err(expected_compound(reader.position())),
    }
}

fn expected_compound(position: usize) -> SnbtError {
    SnbtError {
        kind: SnbtErrorKind::ExpectedCompound,
        position,
    }
}

fn full_text(input: &str) -> Result<(NbtTag, usize), SnbtError> {
    let mut reader = Reader::new(input, 0, Tags);
    let (tag, _) = reader.read()?;
    let end = reader.position();
    reader.skip_whitespace();
    if !reader.rest().is_empty() {
        return Err(SnbtError {
            kind: SnbtErrorKind::Trailing,
            position: reader.position(),
        });
    }
    Ok((tag, end))
}

/// A text that holds one tag and nothing else (SNBT-ARG-004).
pub fn parse_tag(input: &str) -> Result<NbtTag, SnbtError> {
    full_text(input).map(|(tag, _)| tag)
}

/// A text that holds one compound and nothing else (SNBT-ARG-004).
pub fn parse_compound(input: &str) -> Result<NbtCompound, SnbtError> {
    match full_text(input)? {
        (NbtTag::Compound(compound), _) => Ok(compound),
        (_, end) => Err(expected_compound(end)),
    }
}

pub fn write(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_tag(tag, &mut out);
    out
}

pub fn write_compound(compound: &NbtCompound) -> String {
    write(&NbtTag::Compound(compound.clone()))
}

fn write_tag(tag: &NbtTag, out: &mut String) {
    match tag {
        NbtTag::End => out.push_str("END"),
        NbtTag::Byte(v) => _ = write!(out, "{v}b"),
        NbtTag::Short(v) => _ = write!(out, "{v}s"),
        NbtTag::Int(v) => _ = write!(out, "{v}"),
        NbtTag::Long(v) => _ = write!(out, "{v}L"),
        NbtTag::Float(v) => _ = write!(out, "{}f", java_float(*v)),
        NbtTag::Double(v) => _ = write!(out, "{}d", java_double(*v)),
        NbtTag::String(v) => quote_and_escape(v, out),
        NbtTag::ByteArray(v) => {
            out.push_str("[B;");
            for (i, b) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{}B", *b as i8);
            }
            out.push(']');
        }
        NbtTag::IntArray(v) => {
            out.push_str("[I;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{n}");
            }
            out.push(']');
        }
        NbtTag::LongArray(v) => {
            out.push_str("[L;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{n}L");
            }
            out.push(']');
        }
        NbtTag::List(v) => {
            out.push('[');
            for (i, tag) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_tag(tag, out);
            }
            out.push(']');
        }
        NbtTag::Compound(v) => {
            let mut entries: Vec<&(String, NbtTag)> = v.child_tags.iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if is_unquoted_key(key) {
                    out.push_str(key);
                } else {
                    quote_and_escape(key, out);
                }
                out.push(':');
                write_tag(value, out);
            }
            out.push('}');
        }
    }
}

fn is_unquoted_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '.' || first == '_')
        && chars.all(text::is_unquoted)
        && !key.eq_ignore_ascii_case("true")
        && !key.eq_ignore_ascii_case("false")
}

fn quote_and_escape(input: &str, out: &mut String) {
    let mark = out.len();
    out.push(' ');
    let mut quote = None;
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' | '\'' => {
                let chosen = *quote.get_or_insert(if c == '"' { '\'' } else { '"' });
                if chosen == c {
                    out.push('\\');
                }
                out.push(c);
            }
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => _ = write!(out, "\\x{:02X}", c as u32),
            _ => out.push(c),
        }
    }
    let quote = quote.unwrap_or('"');
    out.replace_range(mark..mark + 1, quote.encode_utf8(&mut [0; 4]));
    out.push(quote);
}

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
                out.extend(std::iter::repeat_n('0', point - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
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

/// Five dash-separated hex groups, each group masked to its field width, at
/// most 36 characters in total. Returns the most and least significant halves.
pub fn parse_lenient_uuid(text: &str) -> Option<(i64, i64)> {
    if text.len() > 36 {
        return None;
    }
    let groups: Vec<&str> = text.split('-').collect();
    let [a, b, c, d, e] = groups.as_slice() else {
        return None;
    };
    let hex = |group: &str| i64::from_str_radix(group, 16).ok();
    let most = ((hex(a)? & 0xffff_ffff) << 32) | ((hex(b)? & 0xffff) << 16) | (hex(c)? & 0xffff);
    let least = ((hex(d)? & 0xffff) << 48) | (hex(e)? & 0xffff_ffff_ffff);
    Some((most, least))
}

#[cfg(test)]
mod tests;
