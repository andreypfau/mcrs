use icu_properties::CodePointMapData;
use icu_properties::props::GeneralCategory;

use crate::trim_name;

#[must_use]
pub fn seed_from_text(text: &str) -> Option<i64> {
    let text = trim_name(text);
    if text.is_empty() {
        return None;
    }
    Some(decimal(text).unwrap_or_else(|| text_hash(text)))
}

fn decimal(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for character in digits.chars() {
        let digit = i64::from(digit_value(character)?);
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(digit)?
        } else {
            value.checked_add(digit)?
        };
    }
    Some(value)
}

fn digit_value(character: char) -> Option<u32> {
    let category = CodePointMapData::<GeneralCategory>::new();
    let is_digit = |code: u32| category.get32(code) == GeneralCategory::DecimalNumber;
    let code = u32::from(character);
    if !is_digit(code) {
        return None;
    }
    let preceding = (0..code)
        .rev()
        .take_while(|&before| is_digit(before))
        .count();
    u32::try_from(preceding % 10).ok()
}

fn text_hash(text: &str) -> i64 {
    let hash = text.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    });
    i64::from(hash)
}
